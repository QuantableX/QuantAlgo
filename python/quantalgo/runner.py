"""JSON-RPC runner for QuantAlgo strategies.

Usage
-----
Live / paper mode (reads line-delimited JSON-RPC on stdin):
    python -m quantalgo.runner path/to/strategy.py

Backtest mode (reads a single JSON blob with config + candle data):
    python -m quantalgo.runner path/to/strategy.py --backtest

The runner dynamically imports the first ``Strategy`` subclass found in
the given Python file, instantiates it, then either enters the live
JSON-RPC loop or runs a backtest.
"""

from __future__ import annotations

import argparse
import importlib.util
import inspect
import json
import logging
import sys
import traceback
from pathlib import Path
from typing import Any, Dict, Optional, Type

from quantalgo.backtest_engine import BacktestConfig, BacktestEngine
from quantalgo.models import Candle, Tick, Trade
from quantalgo.strategy import Strategy

# ---------------------------------------------------------------------------
# Logging -- always goes to stderr so stdout stays clean for JSON-RPC.
# ---------------------------------------------------------------------------

logging.basicConfig(
    stream=sys.stderr,
    level=logging.INFO,
    format="[quantalgo] %(levelname)s %(message)s",
)
log = logging.getLogger("quantalgo.runner")


# ---------------------------------------------------------------------------
# Strategy loader
# ---------------------------------------------------------------------------

def _load_strategy_class(path: str) -> Type[Strategy]:
    """Import *path* as a module and return the first Strategy subclass."""
    filepath = Path(path).resolve()
    if not filepath.exists():
        raise FileNotFoundError(f"Strategy file not found: {filepath}")

    spec = importlib.util.spec_from_file_location("_user_strategy", str(filepath))
    if spec is None or spec.loader is None:
        raise ImportError(f"Cannot create module spec for {filepath}")

    module = importlib.util.module_from_spec(spec)
    # Make quantalgo importable inside the user strategy
    sys.modules["_user_strategy"] = module
    spec.loader.exec_module(module)

    for _name, obj in inspect.getmembers(module, inspect.isclass):
        if issubclass(obj, Strategy) and obj is not Strategy:
            return obj

    raise ValueError(
        f"No Strategy subclass found in {filepath}. "
        "Define a class that inherits from quantalgo.Strategy."
    )


# ---------------------------------------------------------------------------
# JSON-RPC helpers
# ---------------------------------------------------------------------------

def _write(msg: dict) -> None:
    """Write a JSON line to stdout and flush."""
    sys.stdout.write(json.dumps(msg) + "\n")
    sys.stdout.flush()


def _read_line() -> Optional[dict]:
    """Read one JSON line from stdin. Returns None on EOF."""
    line = sys.stdin.readline()
    if not line:
        return None
    line = line.strip()
    if not line:
        return None
    return json.loads(line)


# ---------------------------------------------------------------------------
# Live / paper-trading loop
# ---------------------------------------------------------------------------

def _run_live(strategy: Strategy) -> None:
    """Enter the main JSON-RPC read loop."""
    strategy._rpc = _write  # mark as live mode

    log.info("Entering live JSON-RPC loop")

    while True:
        try:
            msg = _read_line()
        except json.JSONDecodeError as exc:
            log.error("Malformed JSON on stdin: %s", exc)
            continue

        if msg is None:
            log.info("stdin closed -- exiting")
            break

        method = msg.get("method")
        params: dict = msg.get("params", {})
        msg_id = msg.get("id")  # may be present for request/response

        try:
            result = _dispatch(strategy, method, params)
            if msg_id is not None:
                _write({"id": msg_id, "result": result})
        except Exception:
            tb = traceback.format_exc()
            log.error("Error handling method=%s:\n%s", method, tb)
            if msg_id is not None:
                _write({
                    "id": msg_id,
                    "error": {"code": -1, "message": tb},
                })


def _dispatch(strategy: Strategy, method: str, params: dict) -> Any:
    """Route an incoming JSON-RPC method to the right handler."""
    if method == "on_candle":
        candle = Candle.from_dict(params)
        strategy._candle_history.append(candle)
        strategy.on_candle(candle)
        return "ok"

    if method == "on_tick":
        tick = Tick.from_dict(params)
        strategy.on_tick(tick)
        return "ok"

    if method == "on_trade":
        trade = Trade.from_dict(params)
        strategy.on_trade(trade)
        return "ok"

    if method == "start":
        # Optionally receive initial balance / positions
        if "balance" in params:
            strategy._balance = params["balance"]
        if "positions" in params:
            from quantalgo.models import Position
            for k, v in params["positions"].items():
                strategy._positions[k] = Position.from_dict(v)
        strategy.on_start()
        return "ok"

    if method == "stop":
        strategy.on_stop()
        return "exit"

    if method == "configure":
        new_params = params.get("params", {})
        strategy.params.update(new_params)
        return "ok"

    log.warning("Unknown method: %s", method)
    return None


# ---------------------------------------------------------------------------
# Backtest mode
# ---------------------------------------------------------------------------

def _run_backtest(strategy: Strategy) -> None:
    """Read a JSON blob from stdin, run the backtest, output results."""
    log.info("Backtest mode -- reading config from stdin")

    raw = sys.stdin.read()
    if not raw.strip():
        log.error("Empty stdin -- expected JSON backtest config")
        sys.exit(1)

    blob: dict = json.loads(raw)

    config_data = blob.get("config", {})
    config = BacktestConfig.from_dict(config_data)

    # Apply strategy params from blob
    strategy_params = blob.get("params", {})
    strategy.params.update(strategy_params)

    candles = blob.get("candles", [])
    if not candles:
        log.error("No candles provided in backtest input")
        sys.exit(1)

    log.info(
        "Running backtest: %d candles, initial_balance=%.2f",
        len(candles),
        config.initial_balance,
    )

    engine = BacktestEngine(strategy, candles, config)
    result = engine.run()

    _write(result)
    log.info("Backtest complete -- %d trades", len(result["trades"]))


# ---------------------------------------------------------------------------
# Entry point
# ---------------------------------------------------------------------------

def main() -> None:
    parser = argparse.ArgumentParser(
        description="QuantAlgo strategy runner",
    )
    parser.add_argument(
        "strategy_path",
        help="Path to the .py file containing a Strategy subclass",
    )
    parser.add_argument(
        "--backtest",
        action="store_true",
        help="Run in backtest mode (read JSON blob from stdin)",
    )
    args = parser.parse_args()

    try:
        cls = _load_strategy_class(args.strategy_path)
    except Exception as exc:
        log.error("Failed to load strategy: %s", exc)
        sys.exit(1)

    strategy = cls()
    log.info("Loaded strategy: %s", cls.__name__)

    if args.backtest:
        _run_backtest(strategy)
    else:
        _run_live(strategy)


if __name__ == "__main__":
    main()

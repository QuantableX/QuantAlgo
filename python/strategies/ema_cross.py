from quantalgo import Strategy


class EMA_Cross(Strategy):
    """EMA crossover strategy — goes long when the fast EMA crosses above
    the slow EMA, goes short when it crosses below, or both.

    The ``direction`` param controls which sides are traded:
      - ``"both"``  — take both long and short entries
      - ``"long"``  — only take long entries
      - ``"short"`` — only take short entries
    """

    params = {
        "fast_period": 12,
        "slow_period": 21,
        "position_size": 0.95,
        "direction": "both",  # "both" | "long" | "short"
    }

    def on_candle(self, candle):
        history = self._candle_history
        if len(history) < self.params["slow_period"] + 1:
            return

        closes = [c.close for c in history]
        fast = self.ema(closes, self.params["fast_period"])
        slow = self.ema(closes, self.params["slow_period"])

        cur_fast, cur_slow = fast[-1], slow[-1]
        prev_fast, prev_slow = fast[-2], slow[-2]

        if None in (cur_fast, cur_slow, prev_fast, prev_slow):
            return

        pair = self._pair
        pos = self.get_position(pair)
        in_position = pos is not None
        direction = self.params["direction"]

        cross_up = prev_fast <= prev_slow and cur_fast > cur_slow
        cross_down = prev_fast >= prev_slow and cur_fast < cur_slow

        # fast crosses above slow
        if cross_up:
            # Reverse short -> long in one host-side operation so sizing uses post-close cash.
            if in_position and getattr(pos, "side", None) == "short":
                if direction in ("both", "long"):
                    self.reverse(pair, "buy", self.params["position_size"])
                else:
                    self.close()
                return
            # open long
            if not in_position and direction in ("both", "long"):
                balance = self.get_balance()
                capital = list(balance.values())[0] if isinstance(balance, dict) else (balance or 0)
                qty = (capital * self.params["position_size"]) / candle.close
                if qty > 0:
                    self.buy(pair, qty)

        # fast crosses below slow
        elif cross_down:
            # Reverse long -> short in one host-side operation so sizing uses post-close cash.
            if in_position and getattr(pos, "side", None) == "long":
                if direction in ("both", "short"):
                    self.reverse(pair, "sell", self.params["position_size"])
                else:
                    self.close()
                return
            # open short
            if not in_position and direction in ("both", "short"):
                balance = self.get_balance()
                capital = list(balance.values())[0] if isinstance(balance, dict) else (balance or 0)
                qty = (capital * self.params["position_size"]) / candle.close
                if qty > 0:
                    self.sell(pair, qty)

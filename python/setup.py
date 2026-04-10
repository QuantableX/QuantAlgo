from setuptools import setup, find_packages

setup(
    name="quantalgo",
    version="0.1.0",
    description="Python strategy SDK for the QuantAlgo crypto trading terminal",
    packages=find_packages(),
    python_requires=">=3.9",
    entry_points={
        "console_scripts": [
            "quantalgo-runner=quantalgo.runner:main",
        ],
    },
)

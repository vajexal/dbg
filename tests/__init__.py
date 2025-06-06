from dataclasses import dataclass


@dataclass(frozen=True)
class Step:
    command: str
    expected_output: str | list[str] = ""
    not_expected_output: str = ""

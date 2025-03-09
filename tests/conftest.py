import os
import pytest


@pytest.fixture(scope="session", autouse=True)
def check_debugger():
    assert os.path.exists("target/debug/dbg")

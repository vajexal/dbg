import subprocess
import string
import random
import os
import pytest

from tests import Step


@pytest.fixture(scope="session", autouse=True)
def check_debugger():
    assert os.path.exists("target/debug/dbg")


@pytest.fixture
def debugger(tmp_path_factory):
    def _debugger(code: str, steps: list[Step], filename: str = ""):
        tmp_path = tmp_path_factory.mktemp("source")
        original_dir = os.getcwd()
        if not filename:
            filename = gen_random_filename()
        src_name = filename + ".c"
        exec_name = filename
        exec_path = os.path.join(tmp_path, exec_name)

        try:
            os.chdir(tmp_path)

            with open(src_name, 'w') as f:
                f.write(code)

            # compile code
            args = ["gcc", "-g", "-O0", "-Wall", src_name, "-o", exec_name]
            subprocess.run(args, stdout=subprocess.PIPE, stderr=subprocess.STDOUT, check=True, text=True)
        except subprocess.CalledProcessError as e:
            pytest.fail(e.stdout)
        finally:
            os.chdir(original_dir)

        # run debugger
        with subprocess.Popen(["target/debug/dbg", exec_path], stdin=subprocess.PIPE, stdout=subprocess.PIPE, stderr=subprocess.STDOUT, text=True) as child:
            for step in steps:
                child.stdin.write(step.command + "\n")
                child.stdin.flush()

                if step.expected_output:
                    if type(step.expected_output) is str:
                        output = child.stdout.readline()
                        assert step.expected_output in output, "expected '{}' in '{}'".format(step.expected_output, output)
                        if step.not_expected_output:
                            assert step.not_expected_output not in output, "not expected '{}' in '{}'".format(step.not_expected_output, output)
                    else:
                        for _ in step.expected_output:
                            output = child.stdout.readline()
                            assert any(expected in output for expected in step.expected_output), "{} not found in {}".format(output, step.expected_output)
                            if step.not_expected_output:
                                assert step.not_expected_output not in output, "not expected '{}' in '{}'".format(step.not_expected_output, output)

            assert child.wait() == 0

    return _debugger


def gen_random_filename() -> str:
    return ''.join(random.choice(string.ascii_lowercase) for _ in range(5))

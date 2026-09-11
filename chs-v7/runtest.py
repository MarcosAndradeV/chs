#!/usr/bin/env python3

import argparse
import os
import subprocess
import sys
from pathlib import Path

# ANSI Escape codes for terminal colors
RED = "\033[31m"
GREEN = "\033[32m"
RESET = "\033[0m"


def main():
    # 1. Parse flags elegantly using argparse
    parser = argparse.ArgumentParser(description="Test runner for the CHS compiler")
    parser.add_argument("-p", "--pause", action="store_true", help="Pause on failure")
    parser.add_argument(
        "-d", "--debug", action="store_true", help="Enable #feature(CHS_DEBUG_ALLOC)"
    )
    parser.add_argument(
        "--gc", action="store_true", help="Enable #feature(CHS_USE_GC)"
    )
    args = parser.parse_args()

    # 2. Set up environment variables
    env = os.environ.copy()
    features = []
    if args.debug:
        features.append("-FCHS_DEBUG_ALLOC")
    if args.gc:
        features.append("-FCHS_USE_GC")
    test_cmd = lambda f: ["target/debug/chs", "run", str(f)] + features

    # 3. Pre-build the compiler
    print("Building compiler...")
    cmd = ["cargo", "build", "-q", "-Fdevelopment"]
    if subprocess.run(cmd, env=env, check=False).returncode != 0:
        print(f"\n{RED}Build failed!{RESET}")
        sys.exit(1)

    print("Testing compiler...")
    if (
        subprocess.run(
            ["cargo", "test", "-q", "--no-fail-fast", "--all"], env=env, check=False
        ).returncode
        != 0
    ):
        print(f"\n{RED}Testing failed!{RESET}")
        sys.exit(1)

    # 4. Initialize counters and tracking
    passed = failed = unexpected_passed = expected_failed = 0
    fail_list = []
    memory_leaks = []

    print("Running tests...")
    print("-" * 16)

    try:
        # --- Handle standard .chs tests ---
        chs_files = list(Path("tests").glob("*.chs"))
        if not chs_files:
            print("No test files found.")
        else:
            for f in chs_files:
                res = subprocess.run(
                    test_cmd(f),
                    env=env,
                    capture_output=True,
                    text=True,
                    check=False,
                )
                if res.stdout:
                    print(res.stdout, end="")
                if res.stderr:
                    print(res.stderr, file=sys.stderr, end="")

                if args.debug and res.stderr and "MEMORY LEAK DETECTED:" in res.stderr:
                    memory_leaks.append(f"Memory leak: {f}")

                if res.returncode == 0:
                    print(f"{GREEN}[ PASS ]{RESET} {f}")
                    passed += 1
                else:
                    print(f"{RED}[ FAIL ]{RESET} {f}")
                    fail_list.append(f"Fail: {f}")
                    failed += 1

                    if args.pause:
                        input(
                            "Press [Enter] to continue to the next test, or Ctrl+C to abort..."
                        )

        # --- Handle .fail tests ---
        fail_files = list(Path("tests").glob("*.fail"))
        if not fail_files:
            print("No fail test files found.")
        else:
            for f in fail_files:
                res = subprocess.run(test_cmd(f), check=False, env=env)
                if res.returncode == 0:
                    print(f"{RED}[ UNEXPECTED PASS ]{RESET} {f}")
                    fail_list.append(f"Unexpected pass: {f}")
                    unexpected_passed += 1

                    if args.pause:
                        input(
                            "Press [Enter] to continue to the next test, or Ctrl+C to abort..."
                        )
                else:
                    print(f"{GREEN}[ EXPECTED FAIL ]{RESET} {f}")
                    expected_failed += 1

    except KeyboardInterrupt:
        # Gracefully handle Ctrl+C without vomiting a stack trace
        print("\nAborted by user.")
        sys.exit(130)

    # 5. Summary output
    print("-" * 16)
    print(
        f"Results: {passed} passed, {failed} failed, {expected_failed} expected to fail, {unexpected_passed} unexpected to pass."
    )

    for item in fail_list:
        print(item)

    for item in memory_leaks:
        print(item)


if __name__ == "__main__":
    main()

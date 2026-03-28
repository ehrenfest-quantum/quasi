#!/usr/bin/env python3
"""quasi-roster CLI: discover, prune, and profile models for the Senate rotation."""

import argparse
import sys

from . import discover, prune, profile


def main():
    parser = argparse.ArgumentParser(
        prog="quasi-roster",
        description="Manage the QUASI Senate rotation roster",
    )
    sub = parser.add_subparsers(dest="command", required=True)

    # discover
    p_disc = sub.add_parser("discover", help="Find new models on providers")
    p_disc.add_argument("--provider", help="Limit to a single provider")
    p_disc.add_argument("--toml", help="Path to rotation.toml")
    p_disc.add_argument("--apply", action="store_true", help="Write changes to TOML")

    # prune
    p_prune = sub.add_parser("prune", help="Quarantine failing/delisted models")
    p_prune.add_argument("--threshold", type=int, default=3, help="Failure threshold (default: 3)")
    p_prune.add_argument("--toml", help="Path to rotation.toml")
    p_prune.add_argument("--apply", action="store_true", help="Write changes to TOML")

    # profile
    p_prof = sub.add_parser("profile", help="Run trial prompts to assign roles")
    p_prof.add_argument("--model", help="Profile a specific model ID")
    p_prof.add_argument("--toml", help="Path to rotation.toml")
    p_prof.add_argument("--apply", action="store_true", help="Write changes to TOML")

    args = parser.parse_args()

    if args.command == "discover":
        discover.run_discover(
            toml_path=args.toml,
            target_provider=args.provider,
            apply=args.apply,
        )
    elif args.command == "prune":
        prune.run_prune(
            toml_path=args.toml,
            threshold=args.threshold,
            apply=args.apply,
        )
    elif args.command == "profile":
        profile.run_profile(
            toml_path=args.toml,
            target_model=args.model,
            apply=args.apply,
        )


if __name__ == "__main__":
    main()

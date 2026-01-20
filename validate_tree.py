#!/usr/bin/env python3
"""
Game Tree Validation Script
Validates the generated game tree for consistency and completeness.
"""

import json
import sys
from typing import Dict, List, Any, Set, Tuple
from dataclasses import dataclass


@dataclass
class ValidationResult:
    passed: bool
    errors: List[str]
    warnings: List[str]
    stats: Dict[str, Any]


class GameTreeValidator:
    REQUIRED_ACTIONS = ["fold", "check", "call", "raise", "allin"]

    def __init__(self, tree: Dict[str, Any]):
        self.tree = tree
        self.nodes = tree.get("nodes", {})
        self.config = tree.get("config", {})
        self.errors: List[str] = []
        self.warnings: List[str] = []

    def validate_node_structure(self, node_name: str, node: Dict[str, Any]) -> bool:
        """Validate a single node has correct structure."""
        valid = True

        # Skip terminal nodes
        if node.get("type") == "terminal":
            return True

        # Check required fields
        if "player" not in node:
            self.errors.append(f"Node '{node_name}' missing 'player' field")
            valid = False

        if "actions" not in node:
            self.errors.append(f"Node '{node_name}' missing 'actions' field")
            valid = False
            return valid

        # Check all 5 action types exist
        actions = node.get("actions", {})
        for action_type in self.REQUIRED_ACTIONS:
            if action_type not in actions:
                self.errors.append(f"Node '{node_name}' missing action type '{action_type}'")
                valid = False

        return valid

    def validate_action_properties(self, node_name: str, node: Dict[str, Any]) -> bool:
        """Validate action properties are correct."""
        valid = True

        if node.get("type") == "terminal":
            return True

        actions = node.get("actions", {})
        state = node.get("state", {})

        for action_type, action_data in actions.items():
            # Every action must have 'enabled' field
            if "enabled" not in action_data:
                self.errors.append(f"Node '{node_name}' action '{action_type}' missing 'enabled' field")
                valid = False
                continue

            # If action is enabled, check required properties
            if action_data.get("enabled"):
                if action_type in ["call", "allin"] and "size" not in action_data:
                    self.warnings.append(f"Node '{node_name}' action '{action_type}' missing 'size'")

                if action_type == "raise":
                    if "sizes" not in action_data:
                        self.errors.append(f"Node '{node_name}' raise action missing 'sizes' array")
                        valid = False
                    elif not isinstance(action_data.get("sizes"), list):
                        self.errors.append(f"Node '{node_name}' raise 'sizes' is not an array")
                        valid = False

        return valid

    def validate_raise_sizes_are_arrays(self) -> bool:
        """Ensure all raise actions have sizes as arrays."""
        valid = True

        for node_name, node in self.nodes.items():
            if node.get("type") == "terminal":
                continue

            actions = node.get("actions", {})
            raise_action = actions.get("raise", {})

            if raise_action.get("enabled"):
                sizes = raise_action.get("sizes")
                if sizes is None:
                    self.errors.append(f"Node '{node_name}': raise enabled but 'sizes' is missing")
                    valid = False
                elif not isinstance(sizes, list):
                    self.errors.append(f"Node '{node_name}': raise 'sizes' must be array, got {type(sizes)}")
                    valid = False
                elif len(sizes) == 0:
                    self.warnings.append(f"Node '{node_name}': raise enabled but 'sizes' is empty")

        return valid

    def validate_commitment_rules(self) -> bool:
        """Verify raise is disabled when player is committed (>30% of stack)."""
        valid = True
        commit_threshold = self.config.get("defaults", {}).get("allin_threshold", 0.30)
        stack = self.config.get("stack", 50.0)

        for node_name, node in self.nodes.items():
            if node.get("type") == "terminal":
                continue

            state = node.get("state", {})
            actions = node.get("actions", {})

            committed_ratio = state.get("committed_ratio", 0)

            if committed_ratio > commit_threshold:
                raise_action = actions.get("raise", {})
                if raise_action.get("enabled"):
                    self.errors.append(
                        f"Node '{node_name}': raise should be disabled when committed "
                        f"({committed_ratio:.2%} > {commit_threshold:.0%})"
                    )
                    valid = False

        return valid

    def validate_disabled_actions(self) -> bool:
        """Verify disabled actions have enabled: false."""
        valid = True

        for node_name, node in self.nodes.items():
            if node.get("type") == "terminal":
                continue

            actions = node.get("actions", {})

            for action_type, action_data in actions.items():
                if "enabled" in action_data and action_data["enabled"] is None:
                    self.errors.append(
                        f"Node '{node_name}' action '{action_type}': enabled should be true/false, not null"
                    )
                    valid = False

        return valid

    def validate_next_references(self) -> bool:
        """Verify all 'next' references point to valid nodes."""
        valid = True
        all_node_names = set(self.nodes.keys())

        for node_name, node in self.nodes.items():
            if node.get("type") == "terminal":
                continue

            actions = node.get("actions", {})

            for action_type, action_data in actions.items():
                if not action_data.get("enabled"):
                    continue

                # Check 'next' field
                next_node = action_data.get("next")
                if next_node and next_node not in all_node_names:
                    self.warnings.append(
                        f"Node '{node_name}' action '{action_type}': "
                        f"'next' references non-existent node '{next_node}'"
                    )

                # Check 'next_map' for raise actions
                if action_type == "raise":
                    next_map = action_data.get("next_map", {})
                    for size, target in next_map.items():
                        if target not in all_node_names:
                            self.warnings.append(
                                f"Node '{node_name}' raise size {size}: "
                                f"'next_map' references non-existent node '{target}'"
                            )

        return valid

    def validate_pot_calculations(self) -> bool:
        """Verify pot calculations are consistent."""
        valid = True
        sb = self.config.get("sb", 0.5)
        bb = self.config.get("bb", 1.0)

        for node_name, node in self.nodes.items():
            if node.get("type") == "terminal":
                continue

            state = node.get("state", {})
            pot = state.get("pot", 0)

            # Basic sanity check - pot should be at least blinds
            if pot < (sb + bb) * 0.9:  # Allow small rounding errors
                self.warnings.append(
                    f"Node '{node_name}': pot ({pot}) is less than blinds ({sb + bb})"
                )

        return valid

    def find_orphan_nodes(self) -> Set[str]:
        """Find nodes that are never referenced."""
        referenced = set()
        all_nodes = set(self.nodes.keys())

        # Starting nodes (open actions)
        for node_name in all_nodes:
            if node_name.startswith("open_"):
                referenced.add(node_name)

        # Find all referenced nodes
        for node_name, node in self.nodes.items():
            if node.get("type") == "terminal":
                continue

            actions = node.get("actions", {})
            for action_type, action_data in actions.items():
                next_node = action_data.get("next")
                if next_node:
                    referenced.add(next_node)

                next_map = action_data.get("next_map", {})
                for target in next_map.values():
                    referenced.add(target)

        return all_nodes - referenced

    def get_statistics(self) -> Dict[str, Any]:
        """Calculate tree statistics."""
        terminal_count = sum(1 for n in self.nodes.values() if n.get("type") == "terminal")
        action_count = len(self.nodes) - terminal_count

        # Count enabled actions
        enabled_actions = {a: 0 for a in self.REQUIRED_ACTIONS}
        for node in self.nodes.values():
            if node.get("type") == "terminal":
                continue
            actions = node.get("actions", {})
            for action_type in self.REQUIRED_ACTIONS:
                if actions.get(action_type, {}).get("enabled"):
                    enabled_actions[action_type] += 1

        # Count unique raise sizes
        all_raise_sizes = set()
        for node in self.nodes.values():
            if node.get("type") == "terminal":
                continue
            raise_action = node.get("actions", {}).get("raise", {})
            if raise_action.get("enabled"):
                for size in raise_action.get("sizes", []):
                    all_raise_sizes.add(size)

        return {
            "total_nodes": len(self.nodes),
            "action_nodes": action_count,
            "terminal_nodes": terminal_count,
            "enabled_actions": enabled_actions,
            "unique_raise_sizes": sorted(all_raise_sizes)
        }

    def validate(self) -> ValidationResult:
        """Run all validations."""
        all_passed = True

        # Structure validation
        for node_name, node in self.nodes.items():
            if not self.validate_node_structure(node_name, node):
                all_passed = False
            if not self.validate_action_properties(node_name, node):
                all_passed = False

        # Specific validations
        if not self.validate_raise_sizes_are_arrays():
            all_passed = False
        if not self.validate_commitment_rules():
            all_passed = False
        if not self.validate_disabled_actions():
            all_passed = False
        if not self.validate_next_references():
            pass  # Warnings only, don't fail
        if not self.validate_pot_calculations():
            pass  # Warnings only, don't fail

        # Check for orphans
        orphans = self.find_orphan_nodes()
        if orphans:
            for orphan in orphans:
                self.warnings.append(f"Orphan node (never referenced): '{orphan}'")

        return ValidationResult(
            passed=all_passed and len(self.errors) == 0,
            errors=self.errors,
            warnings=self.warnings,
            stats=self.get_statistics()
        )


def main():
    import argparse

    parser = argparse.ArgumentParser(description="Validate poker game tree")
    parser.add_argument("--input", default="gametree_v3.json", help="Input tree file")
    parser.add_argument("--verbose", "-v", action="store_true", help="Show warnings")
    args = parser.parse_args()

    # Load tree
    try:
        with open(args.input, 'r') as f:
            tree = json.load(f)
    except FileNotFoundError:
        print(f"Error: File '{args.input}' not found")
        sys.exit(1)
    except json.JSONDecodeError as e:
        print(f"Error: Invalid JSON in '{args.input}': {e}")
        sys.exit(1)

    # Validate
    validator = GameTreeValidator(tree)
    result = validator.validate()

    # Print results
    print(f"\n{'='*60}")
    print(f"Game Tree Validation Report")
    print(f"{'='*60}")
    print(f"File: {args.input}")
    print(f"Version: {tree.get('version', 'unknown')}")
    print()

    # Statistics
    print("Statistics:")
    print(f"  Total nodes: {result.stats['total_nodes']}")
    print(f"  Action nodes: {result.stats['action_nodes']}")
    print(f"  Terminal nodes: {result.stats['terminal_nodes']}")
    print(f"  Unique raise sizes: {result.stats['unique_raise_sizes']}")
    print()

    print("Enabled actions per node type:")
    for action, count in result.stats['enabled_actions'].items():
        print(f"  {action}: {count}")
    print()

    # Errors
    if result.errors:
        print(f"ERRORS ({len(result.errors)}):")
        for error in result.errors:
            print(f"  ❌ {error}")
        print()

    # Warnings
    if args.verbose and result.warnings:
        print(f"WARNINGS ({len(result.warnings)}):")
        for warning in result.warnings:
            print(f"  ⚠️  {warning}")
        print()

    # Final status
    if result.passed:
        print("✅ VALIDATION PASSED")
        sys.exit(0)
    else:
        print("❌ VALIDATION FAILED")
        sys.exit(1)


if __name__ == "__main__":
    main()

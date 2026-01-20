#!/usr/bin/env python3
"""
Game Tree Generator for Poker Solver
Generates a generic game tree with all possible actions at each node.
"""

import json
from dataclasses import dataclass, field
from typing import Dict, List, Optional, Any
from enum import Enum


class Position(Enum):
    UTG = 0
    UTG1 = 1
    LJ = 2
    HJ = 3
    CO = 4
    BTN = 5
    SB = 6
    BB = 7


POSITIONS = ["UTG", "UTG1", "LJ", "HJ", "CO", "BTN", "SB", "BB"]
EP_POSITIONS = ["UTG", "UTG1"]
MP_POSITIONS = ["LJ", "HJ"]
LP_POSITIONS = ["CO", "BTN"]
BLIND_POSITIONS = ["SB", "BB"]


@dataclass
class GameState:
    """Represents the state at any node in the game tree."""
    pot: float
    facing_bet: float
    player_invested: float
    effective_stack: float

    @property
    def to_call(self) -> float:
        return self.facing_bet - self.player_invested

    @property
    def committed_ratio(self) -> float:
        if self.effective_stack == 0:
            return 1.0
        return self.player_invested / self.effective_stack

    @property
    def spr(self) -> float:
        if self.pot == 0:
            return float('inf')
        return self.effective_stack / self.pot


@dataclass
class Config:
    """Configuration loaded from generator_config.json"""
    stack: float = 50.0
    sb: float = 0.5
    bb: float = 1.0
    commit_threshold: float = 0.30
    min_raise_multiplier: float = 2.0

    sizing: Dict[str, List[float]] = field(default_factory=dict)

    @classmethod
    def from_file(cls, path: str) -> 'Config':
        with open(path, 'r') as f:
            data = json.load(f)

        config = cls()
        config.stack = data['config']['stack']
        config.sb = data['config']['sb']
        config.bb = data['config']['bb']
        config.commit_threshold = data['rules']['commit_threshold']
        config.min_raise_multiplier = data['rules']['min_raise_multiplier']
        config.sizing = data['sizing']
        return config


class GameTreeGenerator:
    def __init__(self, config: Config):
        self.config = config
        self.nodes: Dict[str, Any] = {}
        self.terminal_nodes: set = set()

    def get_position_index(self, pos: str) -> int:
        return POSITIONS.index(pos)

    def next_position(self, pos: str) -> Optional[str]:
        """Get next position in action order."""
        idx = self.get_position_index(pos)
        if idx < len(POSITIONS) - 1:
            return POSITIONS[idx + 1]
        return None

    def get_sizing(self, situation: str, raiser_pos: str = None) -> List[float]:
        """Get appropriate sizing for a situation."""
        sizing = self.config.sizing

        if situation == "rfi":
            return sizing.get("rfi", [2.3])
        elif situation == "3bet":
            if raiser_pos in EP_POSITIONS:
                return sizing.get("3bet_vs_ep", [6.9])
            elif raiser_pos in LP_POSITIONS:
                return sizing.get("3bet_vs_lp", [6.9, 9.2])
            else:
                return sizing.get("3bet_from_blinds", [9.2, 10.0])
        elif situation == "4bet":
            return sizing.get("4bet", [16.0, 17.0])
        elif situation == "squeeze":
            return sizing.get("squeeze", [9.2, 10.0])
        elif situation == "bb_vs_sb_limp":
            return sizing.get("bb_vs_sb_limp", [3.5, 6.0])
        elif situation == "bb_vs_sb_raise":
            return sizing.get("bb_vs_sb_raise", [3.5, 4.0])
        elif situation == "sb_vs_bb_3bet":
            return sizing.get("sb_vs_bb_3bet", [10.0, 14.0])
        return [2.3]

    def can_raise(self, state: GameState, min_raise: float) -> bool:
        """Check if player can make a raise."""
        remaining = state.effective_stack - state.player_invested
        return remaining >= min_raise and state.committed_ratio < self.config.commit_threshold

    def generate_actions(self, player: str, state: GameState, situation: str,
                        context: Dict[str, Any]) -> Dict[str, Any]:
        """Generate all possible actions for a node."""
        actions = {}

        # Fold - always available when facing a bet
        actions["fold"] = {
            "enabled": state.facing_bet > state.player_invested,
            "next": context.get("fold_next")
        }

        # Check - available only when no bet to call
        actions["check"] = {
            "enabled": state.facing_bet == state.player_invested,
            "next": context.get("check_next")
        }

        # Call - available when facing a bet
        to_call = state.facing_bet - state.player_invested
        actions["call"] = {
            "enabled": to_call > 0,
            "size": round(to_call, 2) if to_call > 0 else 0,
            "next": context.get("call_next")
        }

        # Raise - available if not committed and can afford
        sizes = self.get_sizing(situation, context.get("original_raiser"))
        min_raise = state.facing_bet * self.config.min_raise_multiplier if state.facing_bet > 0 else self.config.bb * 2
        can_raise = self.can_raise(state, min_raise)

        # Filter sizes to valid ones (less than stack)
        valid_sizes = [s for s in sizes if s <= state.effective_stack - state.player_invested]

        actions["raise"] = {
            "enabled": can_raise and len(valid_sizes) > 0,
            "sizes": valid_sizes if can_raise else [],
            "next_map": context.get("raise_next_map", {})
        }

        # Allin - always available
        allin_size = round(state.effective_stack - state.player_invested, 2)
        actions["allin"] = {
            "enabled": allin_size > state.facing_bet,
            "size": allin_size,
            "next": context.get("allin_next")
        }

        return actions

    def create_node(self, node_name: str, player: str, state: GameState,
                   actions: Dict[str, Any]) -> Dict[str, Any]:
        """Create a node with standard structure."""
        return {
            "player": player,
            "state": {
                "pot": round(state.pot, 2),
                "facing_bet": round(state.facing_bet, 2),
                "player_invested": round(state.player_invested, 2),
                "effective_stack": round(state.effective_stack, 2),
                "committed_ratio": round(state.committed_ratio, 4)
            },
            "actions": actions
        }

    def generate_open_action_tree(self):
        """Generate the opening action tree for each position."""

        # Track who has acted and their actions
        for i, opener in enumerate(POSITIONS[:-1]):  # Everyone except BB can open
            node_name = f"open_{opener.lower()}"

            # Initial state - no one has acted yet
            pot = self.config.sb + self.config.bb  # 1.5 BB

            state = GameState(
                pot=pot,
                facing_bet=self.config.bb,  # BB is the current bet to beat
                player_invested=0 if opener not in BLIND_POSITIONS else (self.config.sb if opener == "SB" else self.config.bb),
                effective_stack=self.config.stack
            )

            # Adjust for blinds
            if opener == "SB":
                state.player_invested = self.config.sb

            # Generate fold chain for positions before opener
            fold_next = f"open_{POSITIONS[i+1].lower()}" if i < len(POSITIONS) - 2 else "sb_completes_or_folds"

            # Call is limping for non-blind positions
            call_next = f"vs_{opener.lower()}_limp_{POSITIONS[i+1].lower()}" if i < len(POSITIONS) - 2 else None

            context = {
                "fold_next": fold_next,
                "check_next": None,  # Can't check in open
                "call_next": call_next,  # Limp
                "raise_next_map": {str(s): f"vs_{opener.lower()}_open_{POSITIONS[i+1].lower()}" for s in self.get_sizing("rfi")},
                "allin_next": f"facing_{opener.lower()}_allin",
                "original_raiser": opener
            }

            actions = self.generate_actions(opener, state, "rfi", context)

            # Adjust - fold shows next position's open, not valid for opener
            actions["fold"]["enabled"] = True
            actions["fold"]["next"] = fold_next

            # Check is fold/pass action for opener
            actions["check"]["enabled"] = False

            self.nodes[node_name] = self.create_node(node_name, opener, state, actions)

    def generate_vs_open_nodes(self):
        """Generate nodes for players facing an open raise."""

        for opener_idx, opener in enumerate(POSITIONS[:-1]):  # Each position that can open
            rfi_size = self.get_sizing("rfi")[0]  # Use first RFI size

            for responder_idx in range(opener_idx + 1, len(POSITIONS)):
                responder = POSITIONS[responder_idx]
                node_name = f"vs_{opener.lower()}_open_{responder.lower()}"

                pot = self.config.sb + self.config.bb + rfi_size

                # Responder's invested amount (blinds if applicable)
                invested = 0
                if responder == "SB":
                    invested = self.config.sb
                elif responder == "BB":
                    invested = self.config.bb

                state = GameState(
                    pot=pot,
                    facing_bet=rfi_size,
                    player_invested=invested,
                    effective_stack=self.config.stack
                )

                # Next player in sequence
                next_pos_idx = responder_idx + 1
                if next_pos_idx < len(POSITIONS):
                    next_pos = POSITIONS[next_pos_idx]
                    fold_next = f"vs_{opener.lower()}_open_{next_pos.lower()}"
                else:
                    fold_next = f"{opener.lower()}_wins"

                # If everyone folds to BB, BB can close action
                if responder == "BB":
                    fold_next = f"{opener.lower()}_wins"

                # 3bet sizing depends on position
                three_bet_sizes = self.get_sizing("3bet", opener)

                context = {
                    "fold_next": fold_next,
                    "check_next": None,
                    "call_next": self._get_call_next(opener, responder, "open"),
                    "raise_next_map": {str(s): f"vs_{responder.lower()}_3bet_{opener.lower()}" for s in three_bet_sizes},
                    "allin_next": f"facing_{responder.lower()}_allin_{opener.lower()}",
                    "original_raiser": opener
                }

                actions = self.generate_actions(responder, state, "3bet", context)
                self.nodes[node_name] = self.create_node(node_name, responder, state, actions)

    def generate_vs_3bet_nodes(self):
        """Generate nodes for original raiser facing a 3bet."""

        for opener_idx, opener in enumerate(POSITIONS[:-1]):
            for three_bettor_idx in range(opener_idx + 1, len(POSITIONS)):
                three_bettor = POSITIONS[three_bettor_idx]

                node_name = f"vs_{three_bettor.lower()}_3bet_{opener.lower()}"

                rfi_size = self.get_sizing("rfi")[0]
                three_bet_size = self.get_sizing("3bet", opener)[0]

                pot = self.config.sb + self.config.bb + rfi_size + three_bet_size

                state = GameState(
                    pot=pot,
                    facing_bet=three_bet_size,
                    player_invested=rfi_size,
                    effective_stack=self.config.stack
                )

                four_bet_sizes = self.get_sizing("4bet")

                context = {
                    "fold_next": f"{three_bettor.lower()}_wins",
                    "check_next": None,
                    "call_next": f"{opener.lower()}_vs_{three_bettor.lower()}_call_flop",
                    "raise_next_map": {str(s): f"vs_{opener.lower()}_4bet_{three_bettor.lower()}" for s in four_bet_sizes},
                    "allin_next": f"facing_{opener.lower()}_allin_{three_bettor.lower()}",
                    "original_raiser": three_bettor
                }

                actions = self.generate_actions(opener, state, "4bet", context)
                self.nodes[node_name] = self.create_node(node_name, opener, state, actions)

    def generate_vs_4bet_nodes(self):
        """Generate nodes for 3bettor facing a 4bet."""

        for opener_idx, opener in enumerate(POSITIONS[:-1]):
            for three_bettor_idx in range(opener_idx + 1, len(POSITIONS)):
                three_bettor = POSITIONS[three_bettor_idx]

                node_name = f"vs_{opener.lower()}_4bet_{three_bettor.lower()}"

                rfi_size = self.get_sizing("rfi")[0]
                three_bet_size = self.get_sizing("3bet", opener)[0]
                four_bet_size = self.get_sizing("4bet")[0]

                pot = self.config.sb + self.config.bb + rfi_size + three_bet_size + four_bet_size

                state = GameState(
                    pot=pot,
                    facing_bet=four_bet_size,
                    player_invested=three_bet_size,
                    effective_stack=self.config.stack
                )

                context = {
                    "fold_next": f"{opener.lower()}_wins",
                    "check_next": None,
                    "call_next": f"{opener.lower()}_vs_{three_bettor.lower()}_4bet_call_flop",
                    "raise_next_map": {},  # 5bet is typically allin at 50bb
                    "allin_next": f"facing_{three_bettor.lower()}_allin_{opener.lower()}_4bet",
                    "original_raiser": opener
                }

                # At 4bet pot, typically committed - check if we should disable raise
                actions = self.generate_actions(three_bettor, state, "5bet", context)
                self.nodes[node_name] = self.create_node(node_name, three_bettor, state, actions)

    def generate_blind_battle_nodes(self):
        """Generate SB vs BB specific scenarios."""

        # SB opens (everyone folds to SB)
        node_name = "sb_open"
        sb_open_sizes = self.get_sizing("rfi")

        state = GameState(
            pot=self.config.sb + self.config.bb,
            facing_bet=self.config.bb,
            player_invested=self.config.sb,
            effective_stack=self.config.stack
        )

        context = {
            "fold_next": "bb_wins",
            "check_next": None,
            "call_next": "sb_limp_bb_action",  # SB limps, BB acts
            "raise_next_map": {str(s): "bb_vs_sb_open" for s in sb_open_sizes},
            "allin_next": "facing_sb_allin_bb",
            "original_raiser": "SB"
        }

        actions = self.generate_actions("SB", state, "rfi", context)
        self.nodes[node_name] = self.create_node(node_name, "SB", state, actions)

        # BB vs SB open (SB raised)
        node_name = "bb_vs_sb_open"
        sb_raise_size = sb_open_sizes[0]

        state = GameState(
            pot=self.config.sb + self.config.bb + sb_raise_size,
            facing_bet=sb_raise_size,
            player_invested=self.config.bb,
            effective_stack=self.config.stack
        )

        bb_3bet_sizes = self.get_sizing("3bet_from_blinds")

        context = {
            "fold_next": "sb_wins",
            "check_next": None,
            "call_next": "sb_vs_bb_call_flop",
            "raise_next_map": {str(s): "sb_vs_bb_3bet" for s in bb_3bet_sizes},
            "allin_next": "facing_bb_allin_sb",
            "original_raiser": "SB"
        }

        actions = self.generate_actions("BB", state, "3bet", context)
        self.nodes[node_name] = self.create_node(node_name, "BB", state, actions)

        # SB vs BB 3bet
        node_name = "sb_vs_bb_3bet"
        bb_3bet_size = bb_3bet_sizes[0]

        state = GameState(
            pot=self.config.sb + self.config.bb + sb_raise_size + bb_3bet_size,
            facing_bet=bb_3bet_size,
            player_invested=sb_raise_size,
            effective_stack=self.config.stack
        )

        sb_4bet_sizes = self.get_sizing("sb_vs_bb_3bet")

        context = {
            "fold_next": "bb_wins",
            "check_next": None,
            "call_next": "sb_vs_bb_3bet_call_flop",
            "raise_next_map": {str(s): "bb_vs_sb_4bet" for s in sb_4bet_sizes},
            "allin_next": "facing_sb_allin_bb_3bet",
            "original_raiser": "BB"
        }

        actions = self.generate_actions("SB", state, "sb_vs_bb_3bet", context)
        self.nodes[node_name] = self.create_node(node_name, "SB", state, actions)

        # BB vs SB 4bet
        node_name = "bb_vs_sb_4bet"
        sb_4bet_size = sb_4bet_sizes[0]

        state = GameState(
            pot=self.config.sb + self.config.bb + sb_raise_size + bb_3bet_size + sb_4bet_size,
            facing_bet=sb_4bet_size,
            player_invested=bb_3bet_size,
            effective_stack=self.config.stack
        )

        context = {
            "fold_next": "sb_wins",
            "check_next": None,
            "call_next": "sb_vs_bb_4bet_call_flop",
            "raise_next_map": {},  # 5bet is allin at this stack depth
            "allin_next": "facing_bb_allin_sb_4bet",
            "original_raiser": "SB"
        }

        actions = self.generate_actions("BB", state, "5bet", context)
        self.nodes[node_name] = self.create_node(node_name, "BB", state, actions)

        # SB limp -> BB action
        node_name = "sb_limp_bb_action"

        state = GameState(
            pot=self.config.sb + self.config.bb,
            facing_bet=self.config.bb,
            player_invested=self.config.bb,
            effective_stack=self.config.stack
        )

        bb_raise_vs_limp_sizes = self.get_sizing("bb_vs_sb_limp")

        context = {
            "fold_next": None,  # Can't fold - no raise
            "check_next": "sb_vs_bb_limp_flop",  # Check = go to flop
            "call_next": None,  # No bet to call
            "raise_next_map": {str(s): "sb_vs_bb_raise_after_limp" for s in bb_raise_vs_limp_sizes},
            "allin_next": "facing_bb_allin_sb_limp",
            "original_raiser": "BB"
        }

        actions = self.generate_actions("BB", state, "bb_vs_sb_limp", context)
        # BB can check or raise when SB limps
        actions["fold"]["enabled"] = False
        actions["check"]["enabled"] = True
        actions["call"]["enabled"] = False

        self.nodes[node_name] = self.create_node(node_name, "BB", state, actions)

        # SB vs BB raise after limp
        node_name = "sb_vs_bb_raise_after_limp"
        bb_raise_size = bb_raise_vs_limp_sizes[0]

        state = GameState(
            pot=self.config.bb + self.config.bb + bb_raise_size,  # SB limped (1bb) + BB's raise
            facing_bet=bb_raise_size,
            player_invested=self.config.bb,  # SB limped in for 1bb
            effective_stack=self.config.stack
        )

        sb_3bet_sizes = self.get_sizing("sb_vs_bb_3bet")

        context = {
            "fold_next": "bb_wins",
            "check_next": None,
            "call_next": "sb_vs_bb_raise_call_flop",
            "raise_next_map": {str(s): "bb_vs_sb_3bet_after_limp" for s in sb_3bet_sizes},
            "allin_next": "facing_sb_allin_bb_raise",
            "original_raiser": "BB"
        }

        actions = self.generate_actions("SB", state, "sb_vs_bb_3bet", context)
        self.nodes[node_name] = self.create_node(node_name, "SB", state, actions)

    def generate_facing_allin_nodes(self):
        """Generate nodes for players facing an all-in."""

        # Generic allin response node structure
        # These are simplified - in real tree you'd have more detail

        allin_scenarios = [
            ("facing_sb_allin_bb", "BB", "SB"),
            ("facing_bb_allin_sb", "SB", "BB"),
            ("facing_bb_allin_sb_3bet", "SB", "BB"),
            ("facing_sb_allin_bb_3bet", "BB", "SB"),
        ]

        for node_name, responder, aggressor in allin_scenarios:
            state = GameState(
                pot=self.config.stack + self.config.stack,  # Max pot
                facing_bet=self.config.stack,
                player_invested=0,  # Simplified
                effective_stack=self.config.stack
            )

            context = {
                "fold_next": f"{aggressor.lower()}_wins",
                "check_next": None,
                "call_next": "showdown",
                "raise_next_map": {},
                "allin_next": None,
                "original_raiser": aggressor
            }

            actions = self.generate_actions(responder, state, "facing_allin", context)
            # Can only fold or call when facing allin
            actions["check"]["enabled"] = False
            actions["raise"]["enabled"] = False
            actions["raise"]["sizes"] = []
            actions["allin"]["enabled"] = False

            self.nodes[node_name] = self.create_node(node_name, responder, state, actions)

    def generate_terminal_nodes(self):
        """Generate terminal nodes (wins, showdowns, flops)."""

        terminals = [
            "showdown",
            "sb_wins",
            "bb_wins",
        ]

        # Position wins
        for pos in POSITIONS:
            terminals.append(f"{pos.lower()}_wins")

        # Flop nodes (terminal for preflop tree)
        flop_scenarios = [
            "sb_vs_bb_call_flop",
            "sb_vs_bb_3bet_call_flop",
            "sb_vs_bb_4bet_call_flop",
            "sb_vs_bb_limp_flop",
            "sb_vs_bb_raise_call_flop",
        ]
        terminals.extend(flop_scenarios)

        for term in terminals:
            if term not in self.nodes:
                self.nodes[term] = {
                    "type": "terminal",
                    "result": term
                }
                self.terminal_nodes.add(term)

    def _get_call_next(self, opener: str, caller: str, situation: str) -> str:
        """Determine next node after a call."""
        opener_idx = self.get_position_index(opener)
        caller_idx = self.get_position_index(caller)

        # If caller is BB, action closes (heads up to flop)
        if caller == "BB":
            return f"{opener.lower()}_vs_{caller.lower()}_call_flop"

        # Otherwise, next position acts
        next_idx = caller_idx + 1
        if next_idx < len(POSITIONS):
            next_pos = POSITIONS[next_idx]
            return f"vs_{opener.lower()}_open_{next_pos.lower()}"

        return f"{opener.lower()}_vs_{caller.lower()}_call_flop"

    def generate(self) -> Dict[str, Any]:
        """Generate the complete game tree."""

        # Generate all node types
        self.generate_open_action_tree()
        self.generate_vs_open_nodes()
        self.generate_vs_3bet_nodes()
        self.generate_vs_4bet_nodes()
        self.generate_blind_battle_nodes()
        self.generate_facing_allin_nodes()
        self.generate_terminal_nodes()

        # Build final tree structure
        tree = {
            "version": "3.0.0",
            "format": "generic_state_machine",
            "config": {
                "seats": 8,
                "stack": self.config.stack,
                "sb": self.config.sb,
                "bb": self.config.bb,
                "positions": POSITIONS,
                "defaults": {
                    "rfi_sizes": self.config.sizing.get("rfi", [2.3]),
                    "3bet_sizes": self.config.sizing.get("3bet_vs_ep", [6.9]),
                    "4bet_sizes": self.config.sizing.get("4bet", [16.0, 17.0]),
                    "allin_threshold": self.config.commit_threshold
                }
            },
            "nodes": self.nodes
        }

        return tree


def main():
    import argparse

    parser = argparse.ArgumentParser(description="Generate poker game tree")
    parser.add_argument("--config", default="generator_config.json", help="Path to config file")
    parser.add_argument("--output", default="gametree_v3.json", help="Output file path")
    args = parser.parse_args()

    # Load config
    config = Config.from_file(args.config)

    # Generate tree
    generator = GameTreeGenerator(config)
    tree = generator.generate()

    # Write output
    with open(args.output, 'w') as f:
        json.dump(tree, f, indent=2)

    print(f"Generated game tree with {len(tree['nodes'])} nodes")
    print(f"Output written to: {args.output}")

    # Stats
    terminal_count = len(generator.terminal_nodes)
    action_count = len(tree['nodes']) - terminal_count
    print(f"  Action nodes: {action_count}")
    print(f"  Terminal nodes: {terminal_count}")


if __name__ == "__main__":
    main()

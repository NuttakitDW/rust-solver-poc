#!/usr/bin/env python3
"""
HRC-style range visualization for poker solver output.
Generates an HTML file showing preflop strategies in a 13x13 grid.
"""

import json
import sys
from pathlib import Path

# Hand names in standard order (rows = first rank, cols = second rank)
RANKS = ['A', 'K', 'Q', 'J', 'T', '9', '8', '7', '6', '5', '4', '3', '2']
RANK_TO_IDX = {r: i for i, r in enumerate(RANKS)}

# Map internal rank (2=0, 3=1, ..., A=12) to display rank
INTERNAL_TO_DISPLAY = {12: 'A', 11: 'K', 10: 'Q', 9: 'J', 8: 'T', 7: '9', 6: '8', 5: '7', 4: '6', 3: '5', 2: '4', 1: '3', 0: '2'}

def bucket_to_hand(bucket: int) -> tuple[str, str]:
    """Convert bucket index to hand name and type (pair/suited/offsuit)."""
    if bucket <= 12:
        # Pairs: 0=22, 12=AA
        rank = INTERNAL_TO_DISPLAY[bucket]
        return f"{rank}{rank}", "pair"
    elif bucket <= 90:
        # Suited: 13-90
        # Formula: 13 + r1 * (r1 - 1) / 2 + r2 where r1 > r2
        idx = bucket - 13
        # Find r1, r2 such that r1*(r1-1)/2 + r2 = idx
        r1 = 1
        while (r1 + 1) * r1 // 2 <= idx:
            r1 += 1
        r2 = idx - r1 * (r1 - 1) // 2
        high = INTERNAL_TO_DISPLAY[r1]
        low = INTERNAL_TO_DISPLAY[r2]
        return f"{high}{low}s", "suited"
    else:
        # Offsuit: 91-168
        idx = bucket - 91
        r1 = 1
        while (r1 + 1) * r1 // 2 <= idx:
            r1 += 1
        r2 = idx - r1 * (r1 - 1) // 2
        high = INTERNAL_TO_DISPLAY[r1]
        low = INTERNAL_TO_DISPLAY[r2]
        return f"{high}{low}o", "offsuit"

def hand_to_grid_pos(hand: str) -> tuple[int, int]:
    """Convert hand name to grid position (row, col)."""
    r1, r2 = hand[0], hand[1]
    row = RANK_TO_IDX[r1]
    col = RANK_TO_IDX[r2]

    if len(hand) == 2:  # Pair
        return row, col
    elif hand[2] == 's':  # Suited - above diagonal
        return min(row, col), max(row, col)
    else:  # Offsuit - below diagonal
        return max(row, col), min(row, col)

def get_cell_name(row: int, col: int) -> str:
    """Get the hand name for a grid cell."""
    r1, r2 = RANKS[row], RANKS[col]
    if row == col:
        return f"{r1}{r2}"
    elif row < col:
        return f"{r1}{r2}s"
    else:
        return f"{r2}{r1}o"

def parse_action(action: str) -> str:
    """Parse action string to category."""
    action_lower = action.lower()
    if 'fold' in action_lower:
        return 'fold'
    elif 'call' in action_lower:
        return 'call'
    elif 'check' in action_lower:
        return 'check'
    elif 'all-in' in action_lower or 'allin' in action_lower:
        return 'allin'
    elif 'raise' in action_lower or 'bet' in action_lower:
        return 'raise'
    return 'other'

def get_action_color(fold_pct: float, call_pct: float, raise_pct: float) -> str:
    """Get background color based on action frequencies."""
    # Color scheme similar to HRC:
    # Red/pink = raise, Green = call, Gray/white = fold

    if raise_pct >= 0.9:
        return '#ff6b6b'  # Strong red - high raise
    elif raise_pct >= 0.7:
        return '#ff8787'  # Medium red
    elif raise_pct >= 0.5:
        return '#ffa8a8'  # Light red
    elif raise_pct >= 0.3:
        return '#ffc9c9'  # Very light red
    elif call_pct >= 0.5:
        return '#8ce99a'  # Green - calling
    elif call_pct >= 0.3:
        return '#b2f2bb'  # Light green
    elif fold_pct >= 0.9:
        return '#e9ecef'  # Gray - folding
    elif fold_pct >= 0.7:
        return '#f1f3f5'  # Light gray
    else:
        return '#fff5f5'  # Mixed - light pink

def generate_html(solution_path: str, output_path: str, history_filter: str = ""):
    """Generate HTML visualization from solution.json."""

    with open(solution_path, 'r') as f:
        data = json.load(f)

    strategies = data.get('strategies', {})
    metadata = data.get('metadata', {})

    # Find all preflop (S0) strategies
    # Group by history to allow filtering
    histories = {}
    for key, strat in strategies.items():
        if strat.get('street', -1) != 0:  # Not preflop
            continue

        history = strat.get('history', '')
        bucket = strat.get('bucket', -1)
        position = strat.get('position', -1)

        hist_key = f"P{position}|{history}"
        if hist_key not in histories:
            histories[hist_key] = {}

        histories[hist_key][bucket] = strat

    # If filter specified, use it; otherwise use first available or empty history for SB (P0)
    if history_filter:
        selected_history = history_filter
    else:
        # Default: SB's opening range (P0 with empty or minimal history)
        sb_histories = [h for h in histories.keys() if h.startswith('P0|') and histories[h]]
        # Prefer empty history or shortest
        sb_histories.sort(key=lambda h: len(h))
        selected_history = sb_histories[0] if sb_histories else list(histories.keys())[0]

    hand_strategies = histories.get(selected_history, {})

    # Build grid data
    grid = [[None for _ in range(13)] for _ in range(13)]

    for bucket, strat in hand_strategies.items():
        hand_name, _ = bucket_to_hand(bucket)
        row, col = hand_to_grid_pos(hand_name)

        actions = strat.get('actions', [])
        probs = strat.get('strategy', [])

        # Categorize actions
        fold_pct = 0.0
        call_pct = 0.0
        raise_pct = 0.0

        for action, prob in zip(actions, probs):
            cat = parse_action(action)
            if cat == 'fold':
                fold_pct += prob
            elif cat in ('call', 'check'):
                call_pct += prob
            elif cat in ('raise', 'allin'):
                raise_pct += prob

        grid[row][col] = {
            'hand': get_cell_name(row, col),
            'fold': fold_pct,
            'call': call_pct,
            'raise': raise_pct,
            'actions': list(zip(actions, probs))
        }

    # Calculate totals
    total_fold = 0
    total_call = 0
    total_raise = 0
    count = 0
    for row in grid:
        for cell in row:
            if cell:
                total_fold += cell['fold']
                total_call += cell['call']
                total_raise += cell['raise']
                count += 1

    if count > 0:
        avg_fold = total_fold / count * 100
        avg_call = total_call / count * 100
        avg_raise = total_raise / count * 100
    else:
        avg_fold = avg_call = avg_raise = 0

    # Parse history for display
    position = "SB" if selected_history.startswith("P0") else "BB"
    history_display = selected_history.split('|')[1] if '|' in selected_history else ""
    if not history_display:
        history_display = "Opening"

    # Generate HTML
    html = f'''<!DOCTYPE html>
<html>
<head>
    <title>Poker Range Visualization</title>
    <style>
        body {{
            font-family: 'Segoe UI', Arial, sans-serif;
            background: #1a1a2e;
            color: #eee;
            padding: 20px;
            margin: 0;
        }}
        .container {{
            max-width: 900px;
            margin: 0 auto;
        }}
        h1 {{
            text-align: center;
            color: #fff;
            margin-bottom: 10px;
        }}
        .metadata {{
            text-align: center;
            color: #888;
            margin-bottom: 20px;
            font-size: 14px;
        }}
        .legend {{
            display: flex;
            justify-content: center;
            gap: 20px;
            margin-bottom: 15px;
            padding: 10px;
            background: #252540;
            border-radius: 8px;
        }}
        .legend-item {{
            display: flex;
            align-items: center;
            gap: 8px;
        }}
        .legend-color {{
            width: 20px;
            height: 20px;
            border-radius: 4px;
        }}
        .grid {{
            display: grid;
            grid-template-columns: repeat(13, 1fr);
            gap: 2px;
            background: #333;
            padding: 2px;
            border-radius: 8px;
        }}
        .cell {{
            aspect-ratio: 1;
            display: flex;
            flex-direction: column;
            justify-content: center;
            align-items: center;
            font-size: 12px;
            font-weight: bold;
            border-radius: 4px;
            cursor: pointer;
            transition: transform 0.1s;
            position: relative;
        }}
        .cell:hover {{
            transform: scale(1.1);
            z-index: 10;
            box-shadow: 0 4px 12px rgba(0,0,0,0.5);
        }}
        .cell .hand {{
            font-size: 14px;
            font-weight: bold;
            color: #333;
        }}
        .cell .pct {{
            font-size: 10px;
            color: #555;
        }}
        .cell.empty {{
            background: #2a2a3e;
        }}
        .cell.empty .hand {{
            color: #666;
        }}
        .history-select {{
            margin-bottom: 20px;
            text-align: center;
        }}
        .history-select select {{
            padding: 8px 16px;
            font-size: 14px;
            background: #252540;
            color: #fff;
            border: 1px solid #444;
            border-radius: 4px;
            cursor: pointer;
        }}
        .tooltip {{
            display: none;
            position: absolute;
            bottom: 105%;
            left: 50%;
            transform: translateX(-50%);
            background: #1a1a2e;
            border: 1px solid #444;
            border-radius: 6px;
            padding: 8px 12px;
            min-width: 150px;
            z-index: 100;
            font-size: 11px;
            color: #fff;
        }}
        .cell:hover .tooltip {{
            display: block;
        }}
        .action-bar {{
            height: 4px;
            display: flex;
            width: 100%;
            position: absolute;
            bottom: 0;
            left: 0;
            border-radius: 0 0 4px 4px;
            overflow: hidden;
        }}
        .action-bar .fold {{ background: #868e96; }}
        .action-bar .call {{ background: #51cf66; }}
        .action-bar .raise {{ background: #ff6b6b; }}
    </style>
</head>
<body>
    <div class="container">
        <h1>Range: {position} {history_display}</h1>
        <div class="metadata">
            Config: {metadata.get('config_name', 'N/A')} |
            Stack: {metadata.get('stack_bb', 'N/A')}bb |
            Iterations: {metadata.get('iterations', 'N/A'):,}
        </div>

        <div class="legend">
            <div class="legend-item">
                <div class="legend-color" style="background: #868e96;"></div>
                <span>Fold ({avg_fold:.1f}%)</span>
            </div>
            <div class="legend-item">
                <div class="legend-color" style="background: #51cf66;"></div>
                <span>Call ({avg_call:.1f}%)</span>
            </div>
            <div class="legend-item">
                <div class="legend-color" style="background: #ff6b6b;"></div>
                <span>Raise ({avg_raise:.1f}%)</span>
            </div>
        </div>

        <div class="history-select">
            <select onchange="window.location.href='?history='+this.value">
                {''.join(f'<option value="{h}" {"selected" if h == selected_history else ""}>{h}</option>' for h in sorted(histories.keys()))}
            </select>
        </div>

        <div class="grid">
'''

    for row in range(13):
        for col in range(13):
            cell = grid[row][col]
            hand_name = get_cell_name(row, col)

            if cell:
                color = get_action_color(cell['fold'], cell['call'], cell['raise'])
                raise_pct = cell['raise'] * 100
                fold_pct = cell['fold'] * 100
                call_pct = cell['call'] * 100

                # Show the dominant action percentage
                if raise_pct >= fold_pct and raise_pct >= call_pct:
                    display_pct = raise_pct
                elif fold_pct >= call_pct:
                    display_pct = fold_pct
                else:
                    display_pct = call_pct

                tooltip_actions = '<br>'.join([f"{a}: {p*100:.1f}%" for a, p in cell['actions']])

                html += f'''            <div class="cell" style="background: {color};">
                <span class="hand">{hand_name}</span>
                <span class="pct">{display_pct:.0f}%</span>
                <div class="action-bar">
                    <div class="fold" style="width: {fold_pct}%;"></div>
                    <div class="call" style="width: {call_pct}%;"></div>
                    <div class="raise" style="width: {raise_pct}%;"></div>
                </div>
                <div class="tooltip">{tooltip_actions}</div>
            </div>
'''
            else:
                html += f'''            <div class="cell empty">
                <span class="hand">{hand_name}</span>
                <span class="pct">-</span>
            </div>
'''

    html += '''        </div>
    </div>
</body>
</html>
'''

    with open(output_path, 'w') as f:
        f.write(html)

    print(f"Generated: {output_path}")
    print(f"Position: {position}, History: {history_display}")
    print(f"Found {len(hand_strategies)} hands for this spot")
    print(f"Available spots: {len(histories)}")

if __name__ == '__main__':
    solution_file = sys.argv[1] if len(sys.argv) > 1 else 'solution.json'
    output_file = sys.argv[2] if len(sys.argv) > 2 else 'range.html'
    history = sys.argv[3] if len(sys.argv) > 3 else ""

    generate_html(solution_file, output_file, history)

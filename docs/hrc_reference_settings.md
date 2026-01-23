# HRC Reference Settings (50bb, 8-max, All Positions)

**Performance Benchmark**: HRC solves all position preflop charts for 50bb in under 2 minutes.

---

## 1. Basic Hand Data

### Stacks and Blinds (8-max)
| Position | Chips | BB |
|----------|-------|-----|
| UTG | 50 | 50.0 |
| EP | 50 | 50.0 |
| MP | 50 | 50.0 |
| HJ | 50 | 50.0 |
| CO | 50 | 50.0 |
| BU | 50 | 50.0 |
| SB | 50 | 50.0 |
| BB | 50 | 50.0 |

### Blind Structure
- **SB**: 0.5
- **BB**: 1
- **Antes**: 0.12 (per player)
- **Straddle**: Off
- **SkipSB**: No
- **Moving BU**: No

### Calculation Mode
- **Hand Mode**: Monte Carlo [Advanced, max. 8 players]
- **Equity Model**: ChipEV in Big Blinds
- **Model Options**: Regular ChipEV(BB)

---

## 2. Preflop Betting Structure

### Opening Sizings
| Context | Sizing |
|---------|--------|
| General | 2.3bb + 1.0bb (per caller) |
| BU (Button) | 2.3bb + 1.0bb |
| SB (Small Blind) | 3.5bb + 1.0bb |
| BB (Big Blind) | 3.5bb + 1.0bb |
| BB vs SB | 3.0bb |

### 3-Bet Sizings
| Context | Sizing |
|---------|--------|
| IP (In Position) | 2.5x + 1.0x |
| BB vs SB | 2.5x |
| BB vs others | 3.3x + 1.0x |
| SB vs BB | 2.6x + 1.0x |
| SB vs others | 3.3x + 1.0x |

### 4-Bet Sizings
| Context | Sizing |
|---------|--------|
| IP | 90.0%, all-in |

### 5-Bet Sizings
| Context | Sizing |
|---------|--------|
| OOP | 120.0%, all-in |

### Limps and Flat Calls
| Action | Allowed Flats |
|--------|---------------|
| Limps | 0 (not allowed) |
| Opens | 1 |
| 3-bet | 1 |
| 4-bet | 1 |
| 5-bet | 0 |

### Additional Options
- **Add SB complete**: Yes (checked)
- **Cold calls**: No (unchecked)
- **Closing flats**: Yes (checked)

### All-in Settings
- **All-in Threshold %**: 40.0
- **Add all-in SPR**: 7.0

---

## 3. Postflop Betting Structure

### Heads-Up Postflop
| Street | Bet | Raise | Donk |
|--------|-----|-------|------|
| Flop | 66% | - | - |
| Turn | - | - | - |
| River | - | - | - |

### Multiway Postflop
| Street | Bet | Raise | Donk |
|--------|-----|-------|------|
| Flop | 66% | - | - |
| Turn | - | - | - |
| River | - | - | - |

### Low SPR Settings
| Setting | Heads-Up | Multiway |
|---------|----------|----------|
| SPR Threshold | 2.5 | 2.5 |
| Bet | 66g (geometric) | 66g |
| Raise | - | - |

### Additional Postflop Settings
- **Donk Bets**: Limited
- **Add all-in SPR**: 5.0

### Final Betting Round (Multiway Depth Limits)
| Players | Final Street |
|---------|--------------|
| 2-way | River |
| 3-way | River |
| 4-way | Turn |
| 5-way | Flop |
| 6-way | Preflop |
| 7-way | Preflop |
| 8-way | Preflop |

---

## 4. Tree Statistics & Abstractions

### Tree Size
- **Total Nodes**: 48,571
- **Total Tree Size**: 0.35 GB
- **HRC Memory Available**: 30.2GB / 31.2GB

### Card Abstractions (Buckets)
| Street | Buckets |
|--------|---------|
| Flop | 1024 |
| Turn | 256 |
| River | 256 |

---

## Key Insights for Implementation

1. **Opening Size**: 2.3bb standard, 3.5bb from blinds
2. **3-Bet Multiplier**: ~2.5-3.3x depending on position
3. **All-in Threshold**: 40% of stack triggers all-in consideration
4. **SPR**: Low SPR (2.5) uses geometric betting
5. **Multiway Simplification**: 6+ players only solve preflop
6. **No Cold Calls**: Simplifies tree significantly
7. **No Limps**: Pure raise/fold preflop strategy
8. **Flop Bet Size**: Standard 66% pot

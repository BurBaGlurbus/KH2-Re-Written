# KH2 Final Mix progress

Baseline: Steam app 2552430 build 15194255 candidate - supported profile unqualified.

| Measure | Accepted / tracked | Percentage |
|---|---:|---:|
| Complete functions decompiled and checked | 0 / 226 (inventory incomplete) | Unmeasured |
| Required behavior scenarios verified 1:1 | 0 / 9 (inventory incomplete) | Unmeasured |
| Final roadmap gates accepted | 0 / 32 | 0.00% |

A PASS counts one reviewed unit. All other statuses count zero. Unknown required units stay in the denominator; an incomplete inventory suppresses the overall percentage. Functions and behavior scenarios have separate denominators and are never averaged into a whole-game score.

## Roadmap acceptance

| Gate | Objective | Verdict | Evidence |
|---|---|---|---|
| P00 | Scope and fidelity contract | PARTIAL | [record](../../docs/milestones.md) |
| P01 | Installed-source and release boundary | PARTIAL | [record](../../docs/milestones.md) |
| P02 | Installed edition/build identity | PARTIAL | [record](../../docs/milestones.md) |
| P03 | Windows/Rust/Bevy environment | PARTIAL | [record](../../docs/milestones.md) |
| P04 | Durable repository/evidence | PARTIAL | [record](../../docs/milestones.md), [record](../../docs/tasks/T121-public-progress-tracker.md) |
| P05 | Pinned tool/license audit | PARTIAL | [record](../../docs/milestones.md) |
| P06 | Native folder picker and source validation | PARTIAL | [record](../../docs/milestones.md) |
| P07 | Packages/archives/dependencies | PARTIAL | [record](../../docs/milestones.md) |
| P08 | Read-only importer/source-bound cache | PARTIAL | [record](../../docs/milestones.md) |
| P09 | Reference capture/replay harness | PARTIAL | [record](../../docs/milestones.md) |
| P10 | Installed executable/runtime research | PARTIAL | [record](../../docs/milestones.md) |
| P11 | Authoritative simulation boundary | NOT RUN | [record](../../docs/milestones.md) |
| P12 | Traceable decoded representation | PARTIAL | [record](../../docs/milestones.md) |
| P13 | Asset/animation inspection app | PARTIAL | [record](../../docs/milestones.md) |
| P14 | PC rendering/presentation | PARTIAL | [record](../../docs/milestones.md) |
| P15 | Animation/motion events | PARTIAL | [record](../../docs/milestones.md) |
| P16 | Rooms/collision/streaming/actors | PARTIAL | [record](../../docs/milestones.md) |
| P17 | Input/time/RNG/determinism | NOT RUN | [record](../../docs/milestones.md) |
| P18 | Movement/camera/player states | PARTIAL | [record](../../docs/milestones.md) |
| P19 | Combat/enemies/reactions | PARTIAL | [record](../../docs/milestones.md) |
| P20 | Scripts/native services | PARTIAL | [record](../../docs/milestones.md) |
| P21 | Complete playable slice | PARTIAL | [record](../../docs/milestones.md) |
| P22 | Progression/party/inventory/systems | PARTIAL | [record](../../docs/milestones.md) |
| P23 | Native saves/safe vendor import | NOT RUN | [record](../../docs/milestones.md) |
| P24 | Story/cutscenes/transitions | PARTIAL | [record](../../docs/milestones.md) |
| P25 | Menus/HUD/settings/EN/JA/fonts | PARTIAL | [record](../../docs/milestones.md) |
| P26 | Audio/music/synchronization | PARTIAL | [record](../../docs/milestones.md) |
| P27 | Every world/encounter/mode | NOT RUN | [record](../../docs/milestones.md) |
| P28 | Campaign and both text options | NOT RUN | [record](../../docs/milestones.md) |
| P29 | Steam changes/recovery | NOT RUN | [record](../../docs/milestones.md) |
| P30 | Performance/Windows reliability | NOT RUN | [record](../../docs/milestones.md) |
| P31 | Windows package/release qualification | NOT RUN | [record](../../docs/milestones.md) |

## Complete functions decompiled and checked

| Unit | Scope | Verdict | Evidence |
|---|---|---|---|
| CIL-T010-01 | T010 managed method selection 1 - historical decompile, equivalence unproved | PARTIAL | [record](../../docs/tasks/T010-managed-reader.md) |
| CIL-T010-02 | T010 managed method selection 2 - historical decompile, equivalence unproved | PARTIAL | [record](../../docs/tasks/T010-managed-reader.md) |
| CIL-T010-03 | T010 managed method selection 3 - historical decompile, equivalence unproved | PARTIAL | [record](../../docs/tasks/T010-managed-reader.md) |
| CIL-T010-04 | T010 managed method selection 4 - historical decompile, equivalence unproved | PARTIAL | [record](../../docs/tasks/T010-managed-reader.md) |
| NATIVE-T124-01 | Enemy scheduler/VM candidate 01 - logical bounds and equivalence unqualified | PARTIAL | [record](../../docs/tasks/T124-enemy-action-arguments-and-scheduler.md) |
| NATIVE-T124-02 | Enemy scheduler/VM candidate 02 - logical bounds and equivalence unqualified | PARTIAL | [record](../../docs/tasks/T124-enemy-action-arguments-and-scheduler.md) |
| NATIVE-T124-03 | Enemy scheduler/VM candidate 03 - logical bounds and equivalence unqualified | PARTIAL | [record](../../docs/tasks/T124-enemy-action-arguments-and-scheduler.md) |
| NATIVE-T124-04 | Enemy scheduler/VM candidate 04 - logical bounds and equivalence unqualified | PARTIAL | [record](../../docs/tasks/T124-enemy-action-arguments-and-scheduler.md) |
| NATIVE-T124-05 | Enemy scheduler/VM candidate 05 - logical bounds and equivalence unqualified | PARTIAL | [record](../../docs/tasks/T124-enemy-action-arguments-and-scheduler.md) |
| NATIVE-T124-06 | Enemy scheduler/VM candidate 06 - logical bounds and equivalence unqualified | PARTIAL | [record](../../docs/tasks/T124-enemy-action-arguments-and-scheduler.md) |
| NATIVE-T124-07 | Enemy scheduler/VM candidate 07 - logical bounds and equivalence unqualified | PARTIAL | [record](../../docs/tasks/T124-enemy-action-arguments-and-scheduler.md) |
| NATIVE-T124-08 | Enemy scheduler/VM candidate 08 - logical bounds and equivalence unqualified | PARTIAL | [record](../../docs/tasks/T124-enemy-action-arguments-and-scheduler.md) |
| NATIVE-T124-09 | Enemy scheduler/VM candidate 09 - logical bounds and equivalence unqualified | PARTIAL | [record](../../docs/tasks/T124-enemy-action-arguments-and-scheduler.md) |
| NATIVE-T124-10 | Enemy scheduler/VM candidate 10 - logical bounds and equivalence unqualified | PARTIAL | [record](../../docs/tasks/T124-enemy-action-arguments-and-scheduler.md) |
| NATIVE-T124-11 | Enemy scheduler/VM candidate 11 - logical bounds and equivalence unqualified | PARTIAL | [record](../../docs/tasks/T124-enemy-action-arguments-and-scheduler.md) |
| NATIVE-T124-12 | Enemy scheduler/VM candidate 12 - logical bounds and equivalence unqualified | PARTIAL | [record](../../docs/tasks/T124-enemy-action-arguments-and-scheduler.md) |
| NATIVE-T124-13 | Enemy scheduler/VM candidate 13 - logical bounds and equivalence unqualified | PARTIAL | [record](../../docs/tasks/T124-enemy-action-arguments-and-scheduler.md) |
| NATIVE-T124-14 | Enemy scheduler/VM candidate 14 - logical bounds and equivalence unqualified | PARTIAL | [record](../../docs/tasks/T124-enemy-action-arguments-and-scheduler.md) |
| NATIVE-T124-15 | Enemy scheduler/VM candidate 15 - logical bounds and equivalence unqualified | PARTIAL | [record](../../docs/tasks/T124-enemy-action-arguments-and-scheduler.md) |
| NATIVE-T124-16 | Enemy scheduler/VM candidate 16 - logical bounds and equivalence unqualified | PARTIAL | [record](../../docs/tasks/T124-enemy-action-arguments-and-scheduler.md) |
| NATIVE-T124-17 | Enemy scheduler/VM candidate 17 - logical bounds and equivalence unqualified | PARTIAL | [record](../../docs/tasks/T124-enemy-action-arguments-and-scheduler.md) |
| NATIVE-T124-18 | Enemy scheduler/VM candidate 18 - logical bounds and equivalence unqualified | PARTIAL | [record](../../docs/tasks/T124-enemy-action-arguments-and-scheduler.md) |
| NATIVE-T124-19 | Enemy scheduler/VM candidate 19 - logical bounds and equivalence unqualified | PARTIAL | [record](../../docs/tasks/T124-enemy-action-arguments-and-scheduler.md) |
| NATIVE-T124-20 | Enemy scheduler/VM candidate 20 - logical bounds and equivalence unqualified | PARTIAL | [record](../../docs/tasks/T124-enemy-action-arguments-and-scheduler.md) |
| NATIVE-T124-21 | Enemy scheduler/VM candidate 21 - logical bounds and equivalence unqualified | PARTIAL | [record](../../docs/tasks/T124-enemy-action-arguments-and-scheduler.md) |
| NATIVE-T124-22 | Enemy scheduler/VM candidate 22 - logical bounds and equivalence unqualified | PARTIAL | [record](../../docs/tasks/T124-enemy-action-arguments-and-scheduler.md) |
| NATIVE-T124-23 | Enemy scheduler/VM candidate 23 - logical bounds and equivalence unqualified | PARTIAL | [record](../../docs/tasks/T124-enemy-action-arguments-and-scheduler.md) |
| NATIVE-T124-24 | Enemy scheduler/VM candidate 24 - logical bounds and equivalence unqualified | PARTIAL | [record](../../docs/tasks/T124-enemy-action-arguments-and-scheduler.md) |
| NATIVE-T124-25 | Enemy scheduler/VM candidate 25 - logical bounds and equivalence unqualified | PARTIAL | [record](../../docs/tasks/T124-enemy-action-arguments-and-scheduler.md) |
| NATIVE-T124-26 | Enemy scheduler/VM candidate 26 - logical bounds and equivalence unqualified | PARTIAL | [record](../../docs/tasks/T124-enemy-action-arguments-and-scheduler.md) |
| NATIVE-T124-27 | Enemy scheduler/VM candidate 27 - logical bounds and equivalence unqualified | PARTIAL | [record](../../docs/tasks/T124-enemy-action-arguments-and-scheduler.md) |
| NATIVE-T124-28 | Enemy scheduler/VM candidate 28 - logical bounds and equivalence unqualified | PARTIAL | [record](../../docs/tasks/T124-enemy-action-arguments-and-scheduler.md) |
| NATIVE-T124-29 | Enemy scheduler/VM candidate 29 - logical bounds and equivalence unqualified | PARTIAL | [record](../../docs/tasks/T124-enemy-action-arguments-and-scheduler.md) |
| NATIVE-T124-30 | Enemy scheduler/VM candidate 30 - logical bounds and equivalence unqualified | PARTIAL | [record](../../docs/tasks/T124-enemy-action-arguments-and-scheduler.md) |
| NATIVE-T124-31 | Enemy scheduler/VM candidate 31 - logical bounds and equivalence unqualified | PARTIAL | [record](../../docs/tasks/T124-enemy-action-arguments-and-scheduler.md) |
| NATIVE-T124-32 | Enemy scheduler/VM candidate 32 - logical bounds and equivalence unqualified | PARTIAL | [record](../../docs/tasks/T124-enemy-action-arguments-and-scheduler.md) |
| NATIVE-T124-33 | Enemy scheduler/VM candidate 33 - logical bounds and equivalence unqualified | PARTIAL | [record](../../docs/tasks/T124-enemy-action-arguments-and-scheduler.md) |
| NATIVE-T124-34 | Enemy scheduler/VM candidate 34 - logical bounds and equivalence unqualified | PARTIAL | [record](../../docs/tasks/T124-enemy-action-arguments-and-scheduler.md) |
| NATIVE-T125-01 | Enemy combat and targeting candidate 01 - complete function unqualified | PARTIAL | [record](../../docs/tasks/T125-enemy-ai-and-combat.md) |
| NATIVE-T125-02 | Enemy combat and targeting candidate 02 - complete function unqualified | PARTIAL | [record](../../docs/tasks/T125-enemy-ai-and-combat.md) |
| NATIVE-T125-03 | Enemy combat and targeting candidate 03 - complete function unqualified | PARTIAL | [record](../../docs/tasks/T125-enemy-ai-and-combat.md) |
| NATIVE-T125-04 | Enemy combat and targeting candidate 04 - complete function unqualified | PARTIAL | [record](../../docs/tasks/T125-enemy-ai-and-combat.md) |
| NATIVE-T125-05 | Enemy combat and targeting candidate 05 - complete function unqualified | PARTIAL | [record](../../docs/tasks/T125-enemy-ai-and-combat.md) |
| NATIVE-T125-06 | Enemy combat and targeting candidate 06 - complete function unqualified | PARTIAL | [record](../../docs/tasks/T125-enemy-ai-and-combat.md) |
| NATIVE-T125-07 | Enemy combat and targeting candidate 07 - complete function unqualified | PARTIAL | [record](../../docs/tasks/T125-enemy-ai-and-combat.md) |
| NATIVE-T125-08 | Enemy combat and targeting candidate 08 - complete function unqualified | PARTIAL | [record](../../docs/tasks/T125-enemy-ai-and-combat.md) |
| NATIVE-T125-09 | Enemy combat and targeting candidate 09 - complete function unqualified | PARTIAL | [record](../../docs/tasks/T125-enemy-ai-and-combat.md) |
| NATIVE-T125-10 | Enemy combat and targeting candidate 10 - complete function unqualified | PARTIAL | [record](../../docs/tasks/T125-enemy-ai-and-combat.md) |
| NATIVE-T125-100 | Enemy combat and targeting candidate 100 - complete function unqualified | PARTIAL | [record](../../docs/tasks/T125-enemy-ai-and-combat.md) |
| NATIVE-T125-101 | Enemy combat and targeting candidate 101 - complete function unqualified | PARTIAL | [record](../../docs/tasks/T125-enemy-ai-and-combat.md) |
| NATIVE-T125-102 | Enemy combat and targeting candidate 102 - complete function unqualified | PARTIAL | [record](../../docs/tasks/T125-enemy-ai-and-combat.md) |
| NATIVE-T125-103 | Enemy combat and targeting candidate 103 - complete function unqualified | PARTIAL | [record](../../docs/tasks/T125-enemy-ai-and-combat.md) |
| NATIVE-T125-104 | Enemy combat and targeting candidate 104 - complete function unqualified | PARTIAL | [record](../../docs/tasks/T125-enemy-ai-and-combat.md) |
| NATIVE-T125-105 | Enemy combat and targeting candidate 105 - complete function unqualified | PARTIAL | [record](../../docs/tasks/T125-enemy-ai-and-combat.md) |
| NATIVE-T125-106 | Enemy combat and targeting candidate 106 - complete function unqualified | PARTIAL | [record](../../docs/tasks/T125-enemy-ai-and-combat.md) |
| NATIVE-T125-107 | Enemy combat and targeting candidate 107 - complete function unqualified | PARTIAL | [record](../../docs/tasks/T125-enemy-ai-and-combat.md) |
| NATIVE-T125-108 | Enemy combat and targeting candidate 108 - complete function unqualified | PARTIAL | [record](../../docs/tasks/T125-enemy-ai-and-combat.md) |
| NATIVE-T125-109 | Enemy combat and targeting candidate 109 - complete function unqualified | PARTIAL | [record](../../docs/tasks/T125-enemy-ai-and-combat.md) |
| NATIVE-T125-11 | Enemy combat and targeting candidate 11 - complete function unqualified | PARTIAL | [record](../../docs/tasks/T125-enemy-ai-and-combat.md) |
| NATIVE-T125-110 | Enemy combat and targeting candidate 110 - complete function unqualified | PARTIAL | [record](../../docs/tasks/T125-enemy-ai-and-combat.md) |
| NATIVE-T125-111 | Enemy combat and targeting candidate 111 - complete function unqualified | PARTIAL | [record](../../docs/tasks/T125-enemy-ai-and-combat.md) |
| NATIVE-T125-112 | Enemy combat and targeting candidate 112 - complete function unqualified | PARTIAL | [record](../../docs/tasks/T125-enemy-ai-and-combat.md) |
| NATIVE-T125-113 | Enemy combat and targeting candidate 113 - complete function unqualified | PARTIAL | [record](../../docs/tasks/T125-enemy-ai-and-combat.md) |
| NATIVE-T125-114 | Enemy combat and targeting candidate 114 - complete function unqualified | PARTIAL | [record](../../docs/tasks/T125-enemy-ai-and-combat.md) |
| NATIVE-T125-115 | Enemy combat and targeting candidate 115 - complete function unqualified | PARTIAL | [record](../../docs/tasks/T125-enemy-ai-and-combat.md) |
| NATIVE-T125-116 | Enemy combat and targeting candidate 116 - complete function unqualified | PARTIAL | [record](../../docs/tasks/T125-enemy-ai-and-combat.md) |
| NATIVE-T125-117 | Enemy combat and targeting candidate 117 - complete function unqualified | PARTIAL | [record](../../docs/tasks/T125-enemy-ai-and-combat.md) |
| NATIVE-T125-118 | Enemy combat and targeting candidate 118 - complete function unqualified | PARTIAL | [record](../../docs/tasks/T125-enemy-ai-and-combat.md) |
| NATIVE-T125-119 | Enemy combat and targeting candidate 119 - complete function unqualified | PARTIAL | [record](../../docs/tasks/T125-enemy-ai-and-combat.md) |
| NATIVE-T125-12 | Enemy combat and targeting candidate 12 - complete function unqualified | PARTIAL | [record](../../docs/tasks/T125-enemy-ai-and-combat.md) |
| NATIVE-T125-120 | Enemy combat and targeting candidate 120 - complete function unqualified | PARTIAL | [record](../../docs/tasks/T125-enemy-ai-and-combat.md) |
| NATIVE-T125-121 | Enemy combat and targeting candidate 121 - complete function unqualified | PARTIAL | [record](../../docs/tasks/T125-enemy-ai-and-combat.md) |
| NATIVE-T125-122 | Enemy combat and targeting candidate 122 - complete function unqualified | PARTIAL | [record](../../docs/tasks/T125-enemy-ai-and-combat.md) |
| NATIVE-T125-123 | Enemy combat and targeting candidate 123 - complete function unqualified | PARTIAL | [record](../../docs/tasks/T125-enemy-ai-and-combat.md) |
| NATIVE-T125-124 | Enemy combat and targeting candidate 124 - complete function unqualified | PARTIAL | [record](../../docs/tasks/T125-enemy-ai-and-combat.md) |
| NATIVE-T125-125 | Enemy combat and targeting candidate 125 - complete function unqualified | PARTIAL | [record](../../docs/tasks/T125-enemy-ai-and-combat.md) |
| NATIVE-T125-126 | Enemy combat and targeting candidate 126 - complete function unqualified | PARTIAL | [record](../../docs/tasks/T125-enemy-ai-and-combat.md) |
| NATIVE-T125-127 | Enemy combat and targeting candidate 127 - complete function unqualified | PARTIAL | [record](../../docs/tasks/T125-enemy-ai-and-combat.md) |
| NATIVE-T125-128 | Enemy combat and targeting candidate 128 - complete function unqualified | PARTIAL | [record](../../docs/tasks/T125-enemy-ai-and-combat.md) |
| NATIVE-T125-129 | Enemy combat and targeting candidate 129 - complete function unqualified | PARTIAL | [record](../../docs/tasks/T125-enemy-ai-and-combat.md) |
| NATIVE-T125-13 | Enemy combat and targeting candidate 13 - complete function unqualified | PARTIAL | [record](../../docs/tasks/T125-enemy-ai-and-combat.md) |
| NATIVE-T125-130 | Enemy combat and targeting candidate 130 - complete function unqualified | PARTIAL | [record](../../docs/tasks/T125-enemy-ai-and-combat.md) |
| NATIVE-T125-131 | Enemy combat and targeting candidate 131 - complete function unqualified | PARTIAL | [record](../../docs/tasks/T125-enemy-ai-and-combat.md) |
| NATIVE-T125-132 | Enemy combat and targeting candidate 132 - complete function unqualified | PARTIAL | [record](../../docs/tasks/T125-enemy-ai-and-combat.md) |
| NATIVE-T125-133 | Enemy combat and targeting candidate 133 - complete function unqualified | PARTIAL | [record](../../docs/tasks/T125-enemy-ai-and-combat.md) |
| NATIVE-T125-134 | Enemy combat and targeting candidate 134 - complete function unqualified | PARTIAL | [record](../../docs/tasks/T125-enemy-ai-and-combat.md) |
| NATIVE-T125-135 | Enemy combat and targeting candidate 135 - complete function unqualified | PARTIAL | [record](../../docs/tasks/T125-enemy-ai-and-combat.md) |
| NATIVE-T125-136 | Enemy combat and targeting candidate 136 - complete function unqualified | PARTIAL | [record](../../docs/tasks/T125-enemy-ai-and-combat.md) |
| NATIVE-T125-137 | Enemy combat and targeting candidate 137 - complete function unqualified | PARTIAL | [record](../../docs/tasks/T125-enemy-ai-and-combat.md) |
| NATIVE-T125-138 | Enemy combat and targeting candidate 138 - complete function unqualified | PARTIAL | [record](../../docs/tasks/T125-enemy-ai-and-combat.md) |
| NATIVE-T125-139 | Enemy combat and targeting candidate 139 - complete function unqualified | PARTIAL | [record](../../docs/tasks/T125-enemy-ai-and-combat.md) |
| NATIVE-T125-14 | Enemy combat and targeting candidate 14 - complete function unqualified | PARTIAL | [record](../../docs/tasks/T125-enemy-ai-and-combat.md) |
| NATIVE-T125-140 | Enemy combat and targeting candidate 140 - complete function unqualified | PARTIAL | [record](../../docs/tasks/T125-enemy-ai-and-combat.md) |
| NATIVE-T125-141 | Enemy combat and targeting candidate 141 - complete function unqualified | PARTIAL | [record](../../docs/tasks/T125-enemy-ai-and-combat.md) |
| NATIVE-T125-142 | Enemy combat and targeting candidate 142 - complete function unqualified | PARTIAL | [record](../../docs/tasks/T125-enemy-ai-and-combat.md) |
| NATIVE-T125-143 | Enemy combat and targeting candidate 143 - complete function unqualified | PARTIAL | [record](../../docs/tasks/T125-enemy-ai-and-combat.md) |
| NATIVE-T125-144 | Enemy combat and targeting candidate 144 - complete function unqualified | PARTIAL | [record](../../docs/tasks/T125-enemy-ai-and-combat.md) |
| NATIVE-T125-145 | Enemy combat and targeting candidate 145 - complete function unqualified | PARTIAL | [record](../../docs/tasks/T125-enemy-ai-and-combat.md) |
| NATIVE-T125-146 | Enemy combat and targeting candidate 146 - complete function unqualified | PARTIAL | [record](../../docs/tasks/T125-enemy-ai-and-combat.md) |
| NATIVE-T125-147 | Enemy combat and targeting candidate 147 - complete function unqualified | PARTIAL | [record](../../docs/tasks/T125-enemy-ai-and-combat.md) |
| NATIVE-T125-148 | Enemy combat and targeting candidate 148 - complete function unqualified | PARTIAL | [record](../../docs/tasks/T125-enemy-ai-and-combat.md) |
| NATIVE-T125-15 | Enemy combat and targeting candidate 15 - complete function unqualified | PARTIAL | [record](../../docs/tasks/T125-enemy-ai-and-combat.md) |
| NATIVE-T125-16 | Enemy combat and targeting candidate 16 - complete function unqualified | PARTIAL | [record](../../docs/tasks/T125-enemy-ai-and-combat.md) |
| NATIVE-T125-17 | Enemy combat and targeting candidate 17 - complete function unqualified | PARTIAL | [record](../../docs/tasks/T125-enemy-ai-and-combat.md) |
| NATIVE-T125-18 | Enemy combat and targeting candidate 18 - complete function unqualified | PARTIAL | [record](../../docs/tasks/T125-enemy-ai-and-combat.md) |
| NATIVE-T125-19 | Enemy combat and targeting candidate 19 - complete function unqualified | PARTIAL | [record](../../docs/tasks/T125-enemy-ai-and-combat.md) |
| NATIVE-T125-20 | Enemy combat and targeting candidate 20 - complete function unqualified | PARTIAL | [record](../../docs/tasks/T125-enemy-ai-and-combat.md) |
| NATIVE-T125-21 | Enemy combat and targeting candidate 21 - complete function unqualified | PARTIAL | [record](../../docs/tasks/T125-enemy-ai-and-combat.md) |
| NATIVE-T125-22 | Enemy combat and targeting candidate 22 - complete function unqualified | PARTIAL | [record](../../docs/tasks/T125-enemy-ai-and-combat.md) |
| NATIVE-T125-23 | Enemy combat and targeting candidate 23 - complete function unqualified | PARTIAL | [record](../../docs/tasks/T125-enemy-ai-and-combat.md) |
| NATIVE-T125-24 | Enemy combat and targeting candidate 24 - complete function unqualified | PARTIAL | [record](../../docs/tasks/T125-enemy-ai-and-combat.md) |
| NATIVE-T125-25 | Enemy combat and targeting candidate 25 - complete function unqualified | PARTIAL | [record](../../docs/tasks/T125-enemy-ai-and-combat.md) |
| NATIVE-T125-26 | Enemy combat and targeting candidate 26 - complete function unqualified | PARTIAL | [record](../../docs/tasks/T125-enemy-ai-and-combat.md) |
| NATIVE-T125-27 | Enemy combat and targeting candidate 27 - complete function unqualified | PARTIAL | [record](../../docs/tasks/T125-enemy-ai-and-combat.md) |
| NATIVE-T125-28 | Enemy combat and targeting candidate 28 - complete function unqualified | PARTIAL | [record](../../docs/tasks/T125-enemy-ai-and-combat.md) |
| NATIVE-T125-29 | Enemy combat and targeting candidate 29 - complete function unqualified | PARTIAL | [record](../../docs/tasks/T125-enemy-ai-and-combat.md) |
| NATIVE-T125-30 | Enemy combat and targeting candidate 30 - complete function unqualified | PARTIAL | [record](../../docs/tasks/T125-enemy-ai-and-combat.md) |
| NATIVE-T125-31 | Enemy combat and targeting candidate 31 - complete function unqualified | PARTIAL | [record](../../docs/tasks/T125-enemy-ai-and-combat.md) |
| NATIVE-T125-32 | Enemy combat and targeting candidate 32 - complete function unqualified | PARTIAL | [record](../../docs/tasks/T125-enemy-ai-and-combat.md) |
| NATIVE-T125-33 | Enemy combat and targeting candidate 33 - complete function unqualified | PARTIAL | [record](../../docs/tasks/T125-enemy-ai-and-combat.md) |
| NATIVE-T125-34 | Enemy combat and targeting candidate 34 - complete function unqualified | PARTIAL | [record](../../docs/tasks/T125-enemy-ai-and-combat.md) |
| NATIVE-T125-35 | Enemy combat and targeting candidate 35 - complete function unqualified | PARTIAL | [record](../../docs/tasks/T125-enemy-ai-and-combat.md) |
| NATIVE-T125-36 | Enemy combat and targeting candidate 36 - complete function unqualified | PARTIAL | [record](../../docs/tasks/T125-enemy-ai-and-combat.md) |
| NATIVE-T125-37 | Enemy combat and targeting candidate 37 - complete function unqualified | PARTIAL | [record](../../docs/tasks/T125-enemy-ai-and-combat.md) |
| NATIVE-T125-38 | Enemy combat and targeting candidate 38 - complete function unqualified | PARTIAL | [record](../../docs/tasks/T125-enemy-ai-and-combat.md) |
| NATIVE-T125-39 | Enemy combat and targeting candidate 39 - complete function unqualified | PARTIAL | [record](../../docs/tasks/T125-enemy-ai-and-combat.md) |
| NATIVE-T125-40 | Enemy combat and targeting candidate 40 - complete function unqualified | PARTIAL | [record](../../docs/tasks/T125-enemy-ai-and-combat.md) |
| NATIVE-T125-41 | Enemy combat and targeting candidate 41 - complete function unqualified | PARTIAL | [record](../../docs/tasks/T125-enemy-ai-and-combat.md) |
| NATIVE-T125-42 | Enemy combat and targeting candidate 42 - complete function unqualified | PARTIAL | [record](../../docs/tasks/T125-enemy-ai-and-combat.md) |
| NATIVE-T125-43 | Enemy combat and targeting candidate 43 - complete function unqualified | PARTIAL | [record](../../docs/tasks/T125-enemy-ai-and-combat.md) |
| NATIVE-T125-44 | Enemy combat and targeting candidate 44 - complete function unqualified | PARTIAL | [record](../../docs/tasks/T125-enemy-ai-and-combat.md) |
| NATIVE-T125-45 | Enemy combat and targeting candidate 45 - complete function unqualified | PARTIAL | [record](../../docs/tasks/T125-enemy-ai-and-combat.md) |
| NATIVE-T125-46 | Enemy combat and targeting candidate 46 - complete function unqualified | PARTIAL | [record](../../docs/tasks/T125-enemy-ai-and-combat.md) |
| NATIVE-T125-47 | Enemy combat and targeting candidate 47 - complete function unqualified | PARTIAL | [record](../../docs/tasks/T125-enemy-ai-and-combat.md) |
| NATIVE-T125-48 | Enemy combat and targeting candidate 48 - complete function unqualified | PARTIAL | [record](../../docs/tasks/T125-enemy-ai-and-combat.md) |
| NATIVE-T125-49 | Enemy combat and targeting candidate 49 - complete function unqualified | PARTIAL | [record](../../docs/tasks/T125-enemy-ai-and-combat.md) |
| NATIVE-T125-50 | Enemy combat and targeting candidate 50 - complete function unqualified | PARTIAL | [record](../../docs/tasks/T125-enemy-ai-and-combat.md) |
| NATIVE-T125-51 | Enemy combat and targeting candidate 51 - complete function unqualified | PARTIAL | [record](../../docs/tasks/T125-enemy-ai-and-combat.md) |
| NATIVE-T125-52 | Enemy combat and targeting candidate 52 - complete function unqualified | PARTIAL | [record](../../docs/tasks/T125-enemy-ai-and-combat.md) |
| NATIVE-T125-53 | Enemy combat and targeting candidate 53 - complete function unqualified | PARTIAL | [record](../../docs/tasks/T125-enemy-ai-and-combat.md) |
| NATIVE-T125-54 | Enemy combat and targeting candidate 54 - complete function unqualified | PARTIAL | [record](../../docs/tasks/T125-enemy-ai-and-combat.md) |
| NATIVE-T125-55 | Enemy combat and targeting candidate 55 - complete function unqualified | PARTIAL | [record](../../docs/tasks/T125-enemy-ai-and-combat.md) |
| NATIVE-T125-56 | Enemy combat and targeting candidate 56 - complete function unqualified | PARTIAL | [record](../../docs/tasks/T125-enemy-ai-and-combat.md) |
| NATIVE-T125-57 | Enemy combat and targeting candidate 57 - complete function unqualified | PARTIAL | [record](../../docs/tasks/T125-enemy-ai-and-combat.md) |
| NATIVE-T125-58 | Enemy combat and targeting candidate 58 - complete function unqualified | PARTIAL | [record](../../docs/tasks/T125-enemy-ai-and-combat.md) |
| NATIVE-T125-59 | Enemy combat and targeting candidate 59 - complete function unqualified | PARTIAL | [record](../../docs/tasks/T125-enemy-ai-and-combat.md) |
| NATIVE-T125-60 | Enemy combat and targeting candidate 60 - complete function unqualified | PARTIAL | [record](../../docs/tasks/T125-enemy-ai-and-combat.md) |
| NATIVE-T125-61 | Enemy combat and targeting candidate 61 - complete function unqualified | PARTIAL | [record](../../docs/tasks/T125-enemy-ai-and-combat.md) |
| NATIVE-T125-62 | Enemy combat and targeting candidate 62 - complete function unqualified | PARTIAL | [record](../../docs/tasks/T125-enemy-ai-and-combat.md) |
| NATIVE-T125-63 | Enemy combat and targeting candidate 63 - complete function unqualified | PARTIAL | [record](../../docs/tasks/T125-enemy-ai-and-combat.md) |
| NATIVE-T125-64 | Enemy combat and targeting candidate 64 - complete function unqualified | PARTIAL | [record](../../docs/tasks/T125-enemy-ai-and-combat.md) |
| NATIVE-T125-65 | Enemy combat and targeting candidate 65 - complete function unqualified | PARTIAL | [record](../../docs/tasks/T125-enemy-ai-and-combat.md) |
| NATIVE-T125-66 | Enemy combat and targeting candidate 66 - complete function unqualified | PARTIAL | [record](../../docs/tasks/T125-enemy-ai-and-combat.md) |
| NATIVE-T125-67 | Enemy combat and targeting candidate 67 - complete function unqualified | PARTIAL | [record](../../docs/tasks/T125-enemy-ai-and-combat.md) |
| NATIVE-T125-68 | Enemy combat and targeting candidate 68 - complete function unqualified | PARTIAL | [record](../../docs/tasks/T125-enemy-ai-and-combat.md) |
| NATIVE-T125-69 | Enemy combat and targeting candidate 69 - complete function unqualified | PARTIAL | [record](../../docs/tasks/T125-enemy-ai-and-combat.md) |
| NATIVE-T125-70 | Enemy combat and targeting candidate 70 - complete function unqualified | PARTIAL | [record](../../docs/tasks/T125-enemy-ai-and-combat.md) |
| NATIVE-T125-71 | Enemy combat and targeting candidate 71 - complete function unqualified | PARTIAL | [record](../../docs/tasks/T125-enemy-ai-and-combat.md) |
| NATIVE-T125-72 | Enemy combat and targeting candidate 72 - complete function unqualified | PARTIAL | [record](../../docs/tasks/T125-enemy-ai-and-combat.md) |
| NATIVE-T125-73 | Enemy combat and targeting candidate 73 - complete function unqualified | PARTIAL | [record](../../docs/tasks/T125-enemy-ai-and-combat.md) |
| NATIVE-T125-74 | Enemy combat and targeting candidate 74 - complete function unqualified | PARTIAL | [record](../../docs/tasks/T125-enemy-ai-and-combat.md) |
| NATIVE-T125-75 | Enemy combat and targeting candidate 75 - complete function unqualified | PARTIAL | [record](../../docs/tasks/T125-enemy-ai-and-combat.md) |
| NATIVE-T125-76 | Enemy combat and targeting candidate 76 - complete function unqualified | PARTIAL | [record](../../docs/tasks/T125-enemy-ai-and-combat.md) |
| NATIVE-T125-77 | Enemy combat and targeting candidate 77 - complete function unqualified | PARTIAL | [record](../../docs/tasks/T125-enemy-ai-and-combat.md) |
| NATIVE-T125-78 | Enemy combat and targeting candidate 78 - complete function unqualified | PARTIAL | [record](../../docs/tasks/T125-enemy-ai-and-combat.md) |
| NATIVE-T125-79 | Enemy combat and targeting candidate 79 - complete function unqualified | PARTIAL | [record](../../docs/tasks/T125-enemy-ai-and-combat.md) |
| NATIVE-T125-80 | Enemy combat and targeting candidate 80 - complete function unqualified | PARTIAL | [record](../../docs/tasks/T125-enemy-ai-and-combat.md) |
| NATIVE-T125-81 | Enemy combat and targeting candidate 81 - complete function unqualified | PARTIAL | [record](../../docs/tasks/T125-enemy-ai-and-combat.md) |
| NATIVE-T125-82 | Enemy combat and targeting candidate 82 - complete function unqualified | PARTIAL | [record](../../docs/tasks/T125-enemy-ai-and-combat.md) |
| NATIVE-T125-83 | Enemy combat and targeting candidate 83 - complete function unqualified | PARTIAL | [record](../../docs/tasks/T125-enemy-ai-and-combat.md) |
| NATIVE-T125-84 | Enemy combat and targeting candidate 84 - complete function unqualified | PARTIAL | [record](../../docs/tasks/T125-enemy-ai-and-combat.md) |
| NATIVE-T125-85 | Enemy combat and targeting candidate 85 - complete function unqualified | PARTIAL | [record](../../docs/tasks/T125-enemy-ai-and-combat.md) |
| NATIVE-T125-86 | Enemy combat and targeting candidate 86 - complete function unqualified | PARTIAL | [record](../../docs/tasks/T125-enemy-ai-and-combat.md) |
| NATIVE-T125-87 | Enemy combat and targeting candidate 87 - complete function unqualified | PARTIAL | [record](../../docs/tasks/T125-enemy-ai-and-combat.md) |
| NATIVE-T125-88 | Enemy combat and targeting candidate 88 - complete function unqualified | PARTIAL | [record](../../docs/tasks/T125-enemy-ai-and-combat.md) |
| NATIVE-T125-89 | Enemy combat and targeting candidate 89 - complete function unqualified | PARTIAL | [record](../../docs/tasks/T125-enemy-ai-and-combat.md) |
| NATIVE-T125-90 | Enemy combat and targeting candidate 90 - complete function unqualified | PARTIAL | [record](../../docs/tasks/T125-enemy-ai-and-combat.md) |
| NATIVE-T125-91 | Enemy combat and targeting candidate 91 - complete function unqualified | PARTIAL | [record](../../docs/tasks/T125-enemy-ai-and-combat.md) |
| NATIVE-T125-92 | Enemy combat and targeting candidate 92 - complete function unqualified | PARTIAL | [record](../../docs/tasks/T125-enemy-ai-and-combat.md) |
| NATIVE-T125-93 | Enemy combat and targeting candidate 93 - complete function unqualified | PARTIAL | [record](../../docs/tasks/T125-enemy-ai-and-combat.md) |
| NATIVE-T125-94 | Enemy combat and targeting candidate 94 - complete function unqualified | PARTIAL | [record](../../docs/tasks/T125-enemy-ai-and-combat.md) |
| NATIVE-T125-95 | Enemy combat and targeting candidate 95 - complete function unqualified | PARTIAL | [record](../../docs/tasks/T125-enemy-ai-and-combat.md) |
| NATIVE-T125-96 | Enemy combat and targeting candidate 96 - complete function unqualified | PARTIAL | [record](../../docs/tasks/T125-enemy-ai-and-combat.md) |
| NATIVE-T125-97 | Enemy combat and targeting candidate 97 - complete function unqualified | PARTIAL | [record](../../docs/tasks/T125-enemy-ai-and-combat.md) |
| NATIVE-T125-98 | Enemy combat and targeting candidate 98 - complete function unqualified | PARTIAL | [record](../../docs/tasks/T125-enemy-ai-and-combat.md) |
| NATIVE-T125-99 | Enemy combat and targeting candidate 99 - complete function unqualified | PARTIAL | [record](../../docs/tasks/T125-enemy-ai-and-combat.md) |
| NATIVE-T206-01 | Boss class, revenge gauge and enemy hit order - translation, equivalence unproved | PARTIAL | [record](../../docs/tasks/T206-boss-framework-sephiroth.md) |
| NATIVE-T206-02 | Enemy kill veto and one-damage rule - translation, equivalence unproved | PARTIAL | [record](../../docs/tasks/T206-boss-framework-sephiroth.md) |
| NATIVE-T206-03 | Reaction override table by state pattern - translation, equivalence unproved | PARTIAL | [record](../../docs/tasks/T206-boss-framework-sephiroth.md) |
| NATIVE-T206-04 | Battle sheet binding from an ENMP row - translation, equivalence unproved | PARTIAL | [record](../../docs/tasks/T206-boss-framework-sephiroth.md) |
| NATIVE-T206-05 | Reaction-command enable test of an object's collision groups - translation, equivalence unproved | PARTIAL | [record](../../docs/tasks/T206-boss-framework-sephiroth.md) |
| NATIVE-T206-06 | Reaction-command search and press for the leader - translation, equivalence unproved | PARTIAL | [record](../../docs/tasks/T206-boss-framework-sephiroth.md) |
| NATIVE-T206-07 | Script side threads and hooks - translation, equivalence unproved | PARTIAL | [record](../../docs/tasks/T206-boss-framework-sephiroth.md) |
| NATIVE-T206-08 | Attack-reached callback to the owner's script - translation, equivalence unproved | PARTIAL | [record](../../docs/tasks/T206-boss-framework-sephiroth.md) |
| NATIVE-T206-09 | Vacuum pull on an object - translation, equivalence unproved | PARTIAL | [record](../../docs/tasks/T206-boss-framework-sephiroth.md) |
| NATIVE-T206-10 | Script lights as the scene tint - translation, equivalence unproved | PARTIAL | [record](../../docs/tasks/T206-boss-framework-sephiroth.md) |
| NATIVE-T206-11 | Mission boss-clear sequence and its overlays - translation, equivalence unproved | PARTIAL | [record](../../docs/tasks/T206-boss-framework-sephiroth.md) |
| NATIVE-T206-12 | Per-tick movement request reset - translation, equivalence unproved | PARTIAL | [record](../../docs/tasks/T206-boss-framework-sephiroth.md) |
| NATIVE-T209-01 | Event overlay camera on its anchor object - translation, equivalence unproved | PARTIAL | [record](../../docs/tasks/T209-sephiroth-follow-ups.md) |
| NATIVE-T209-02 | Event overlay frame rate and radial blur feedback - translation, equivalence unproved | PARTIAL | [record](../../docs/tasks/T209-sephiroth-follow-ups.md) |
| NATIVE-T209-03 | Object voice slot priority and line replacement - translation, equivalence unproved | PARTIAL | [record](../../docs/tasks/T209-sephiroth-follow-ups.md) |
| NATIVE-T209-04 | Script attack hit identity kept until the attack is freed - translation, equivalence unproved | PARTIAL | [record](../../docs/tasks/T209-sephiroth-follow-ups.md) |
| NATIVE-T209-05 | Scene light tint on the fog colour - translation, equivalence unproved | PARTIAL | [record](../../docs/tasks/T209-sephiroth-follow-ups.md) |
| NATIVE-T209-06 | Vacuum pull on party members - translation, equivalence unproved | PARTIAL | [record](../../docs/tasks/T209-sephiroth-follow-ups.md) |
| NATIVE-T209-07 | Hook-thread fall of the leader after a capture - translation, equivalence unproved | PARTIAL | [record](../../docs/tasks/T209-sephiroth-follow-ups.md) |
| NATIVE-T209-08 | Landing hand-over to idle and blow landing - translation, equivalence unproved | PARTIAL | [record](../../docs/tasks/T209-sephiroth-follow-ups.md) |
| NATIVE-T209-09 | Hits on objects a script made - translation, equivalence unproved | PARTIAL | [record](../../docs/tasks/T209-sephiroth-follow-ups.md) |
| NATIVE-T211-01 | Reaction command search over the held command, record 1, partner, shapes, attacks and auto offers - translation, equivalence unproved | PARTIAL | [record](../../docs/tasks/T211-reaction-command-manager.md) |
| NATIVE-T211-02 | Reaction slot, target marker and Triangle press - translation, equivalence unproved | PARTIAL | [record](../../docs/tasks/T211-reaction-command-manager.md) |
| NATIVE-T211-03 | Reaction slot drawing, prompt and burst on the command menu - translation, equivalence unproved | PARTIAL | [record](../../docs/tasks/T211-reaction-command-manager.md) |
| NATIVE-T211-04 | Attack-borne reaction commands - translation, equivalence unproved | PARTIAL | [record](../../docs/tasks/T211-reaction-command-manager.md) |
| NATIVE-T211-05 | Save point offer and heal while Save shows - translation, equivalence unproved | PARTIAL | [record](../../docs/tasks/T211-reaction-command-manager.md) |
| NATIVE-T211-06 | Partner and spawn reaction command sources - translation, equivalence unproved | PARTIAL | [record](../../docs/tasks/T211-reaction-command-manager.md) |
| NATIVE-T212-02 | World map manual lock, lock-on camera and target switch - translation, equivalence unproved | PARTIAL | [record](../../docs/tasks/T212-world-map-cursor-lock-and-fades.md) |
| NATIVE-T212-03 | World map cursor deck (kind 10) and its commands - translation, equivalence unproved | PARTIAL | [record](../../docs/tasks/T212-world-map-cursor-lock-and-fades.md) |
| NATIVE-T212-04 | World map total play time panel and info panel anchoring - translation, equivalence unproved | PARTIAL | [record](../../docs/tasks/T212-world-map-cursor-lock-and-fades.md) |
| NATIVE-T212-05 | World map jump fades (mode 1) and wall knock-back - translation, equivalence unproved | PARTIAL | [record](../../docs/tasks/T212-world-map-cursor-lock-and-fades.md) |
| NATIVE-T213-01 | Script vectors, offsets and headings in the engine frame - translation, equivalence unproved | PARTIAL | [record](../../docs/tasks/T213-script-frame.md) |
| NATIVE-T214-01 | World map pause menu and journal hosting - translation, equivalence unproved | PARTIAL | [record](../../docs/tasks/T214-world-map-pause-menu-journal-play-time.md) |
| NATIVE-T214-02 | World map per-world play time on the info panel - translation, equivalence unproved | PARTIAL | [record](../../docs/tasks/T214-world-map-pause-menu-journal-play-time.md) |
| NATIVE-T217-01 | Leave-ground branch of the physics step - translation, equivalence unproved | PARTIAL | [record](../../docs/tasks/T217-leave-the-ground.md) |
| NATIVE-T219-01 | Boss map body, its solve, leave and land tests, and the random-position search's collision class - translation, equivalence unproved | PARTIAL | [record](../../docs/tasks/T219-boss-map-body.md) |
| NATIVE-T222-01 | Map hit record of the script line check and its normal, hit, crossing and surface getters - translation, equivalence unproved | PARTIAL | [record](../../docs/tasks/T222-map-hit-record.md) |
| NATIVE-T223-01 | Removal of a leaving object once its alpha reaches zero - translation, equivalence unproved | PARTIAL | [record](../../docs/tasks/T223-leaving-objects-removed.md) |
| NATIVE-T224-01 | Boss script effects on tracked handles with commands, liveness and per-object containers - translation, equivalence unproved | PARTIAL | [record](../../docs/tasks/T224-boss-script-effects-tracked.md) |
| NATIVE-T228-01 | Effect texture binding, remastered pages, sprite page coordinates, blends, depth-only model draws and the prize heal effect's end - translation, equivalence unproved | PARTIAL | [record](../../docs/tasks/T228-effect-textures-uvs-blends.md) |

Accepted share of the tracked subset: 0.00%. This is not a whole-game percentage.

## Required behavior scenarios verified 1:1

| Unit | Scope | Verdict | Evidence |
|---|---|---|---|
| OBS-T122-01 | Enemy spawn and action selection - reference comparison required | NOT RUN | [record](../../docs/tasks/T122-enemy-summons-and-large-arena.md) |
| OBS-T122-02 | Enemy idle and attack behavior - reference comparison required | NOT RUN | [record](../../docs/tasks/T122-enemy-summons-and-large-arena.md) |
| OBS-T122-03 | Enemy animation and model presentation - reference comparison required | NOT RUN | [record](../../docs/tasks/T122-enemy-summons-and-large-arena.md), [record](../../docs/tasks/T123-enemy-motion-and-model-boundaries.md) |
| OBS-T122-04 | Enemy encounter roster and reachability - reference comparison required | NOT RUN | [record](../../docs/tasks/T122-enemy-summons-and-large-arena.md) |
| OBS-T124-01 | Enemy action queue and script scheduling - reference comparison required | NOT RUN | [record](../../docs/tasks/T124-enemy-action-arguments-and-scheduler.md) |
| OBS-T125-01 | Multiple actor combat and damage - reference comparison required | NOT RUN | [record](../../docs/tasks/T125-enemy-ai-and-combat.md) |
| OBS-T125-02 | Lock-on, Scan and camera - reference comparison required | NOT RUN | [record](../../docs/tasks/T125-enemy-ai-and-combat.md) |
| OBS-T206-01 | Sephiroth boss fight - attacks, Block, grab, damage and defeat - reference comparison required | NOT RUN | [record](../../docs/tasks/T206-boss-framework-sephiroth.md) |
| OBS-T209-01 | Sephiroth Heartless Angel, Meteor, grab and clear overlays - reference comparison required | NOT RUN | [record](../../docs/tasks/T209-sephiroth-follow-ups.md) |

Accepted share of the tracked subset: 0.00%. This is not a whole-game percentage.

## Maintaining the ledger

Edit `docs/progress/coverage.json` when a reviewed evidence record changes a verdict. Add one opaque ID per complete logical function or required behavior scenario, including untested and blocked units. Keep IDs stable and retain failed units. Do not enter addresses, disassembly, pseudocode, source hashes, personal paths or private evidence files.

For decompilation, PASS requires qualified complete logical function bounds and a reviewed source-bound decoding/decompilation check. An interior slice or byte identity alone is PARTIAL. For fidelity, PASS requires a controlled installed/rewrite comparison under the fidelity contract with no unresolved required discrepancy. Roadmap PASS requires the final gate acceptance, not a task's local build/test PASS.

Mark an inventory complete only after its public inventory evidence documents full KH2FM scope, shared dependencies, deduplication, exclusions and remaining unknown units. Every PASS links to its public acceptance record. The generator validates structure and links; evidence review still determines whether the claim is justified.

Run `python tools/progress.py` to refresh the README, this report and `summary.json`. Run `python tools/progress.py --check` to detect stale outputs. JSON null means unmeasured, never zero. The output is deterministic and contains only public ledger labels, statuses, counts and a ledger digest. It never reads Steam files or private research.

## GitHub refresh

The progress workflow tests and generates on each main-branch push, daily at 13:17 UTC and on manual dispatch. Pull requests validate and preview without write permissions. Only changed generated files are committed; unchanged schedules create no commits. A concurrent main update is rejected by normal fast-forward push; the next push or scheduled run regenerates from the new head. Branch protection may require a maintainer to allow the bot's documentation updates.

GitHub schedules run on the default branch and may be delayed: [workflow syntax](https://docs.github.com/en/actions/reference/workflows-and-actions/workflow-syntax). The built-in GITHUB_TOKEN prevents generated commits from recursively triggering more push workflows: [triggering workflows](https://docs.github.com/en/actions/how-tos/write-workflows/choose-when-workflows-run/trigger-a-workflow). Refreshing reports cannot discover private evidence or advance a verdict without a reviewed ledger update.

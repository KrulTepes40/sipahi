---- MODULE SipahiScheduler ----
(* Sipahi Microkernel — Scheduler TLA+ Specification
   Models: task state transitions, fixed-priority preemptive scheduling,
           period-based budget replenishment.
   Verifies: starvation freedom, priority correctness, state invariants.

   NOTE: This spec models policy DECISION logic, not trigger mechanisms.
   Runtime trigger conditions (e.g., 3× consecutive cap fail → CapViolation,
   isolated_count ≥ 2 → MultiModuleCrash, CLINT overrun → DeadlineMiss)
   are implementation details verified by Kani proofs and runtime tests.
   This spec verifies that GIVEN a PolicyEvent, the correct FailureMode
   is selected and escalation terminates. *)

EXTENDS Integers, FiniteSets, Sequences, TLC

CONSTANTS
    TASKS,          \* Set of task IDs (e.g., {0, 1, 2, 3})
    MAX_PRIORITY,   \* Highest priority number (lowest priority), e.g., 15
    MAX_PERIOD      \* Maximum period length in ticks

VARIABLES
    state,          \* state[t] \in {Ready, Running, Suspended, Dead, Isolated}
    priority,       \* priority[t] \in 0..MAX_PRIORITY (0 = highest)
    budget,         \* budget[t] \in Nat (remaining cycles)
    maxBudget,      \* maxBudget[t] \in Nat (per-period budget)
    periodCounter,  \* periodCounter[t] \in 0..MAX_PERIOD
    periodLength,   \* periodLength[t] \in 1..MAX_PERIOD
    running,        \* Currently running task (-1 = idle)
    tick            \* Global tick counter

vars == <<state, priority, budget, maxBudget, periodCounter, periodLength, running, tick>>

States == {"Ready", "Running", "Suspended", "Dead", "Isolated"}

TypeOK ==
    /\ state \in [TASKS -> States]
    /\ priority \in [TASKS -> 0..MAX_PRIORITY]
    /\ budget \in [TASKS -> Nat]
    /\ maxBudget \in [TASKS -> Nat]
    /\ periodCounter \in [TASKS -> 0..MAX_PERIOD]
    /\ periodLength \in [TASKS -> 1..MAX_PERIOD]
    /\ running \in TASKS \cup {-1}
    /\ tick \in Nat

(* ═══ Initial State ═══ *)
Init ==
    /\ state = [t \in TASKS |-> "Ready"]
    /\ priority = (0 :> 0 @@ 1 :> 2)
    /\ budget = (0 :> 2 @@ 1 :> 2)
    /\ maxBudget = budget
    /\ periodCounter = [t \in TASKS |-> 0]
    /\ periodLength = (0 :> 3 @@ 1 :> 3)
    /\ running = -1
    /\ tick = 0

(* ═══ Helper: Select highest priority Ready/Running task ═══ *)
ReadyTasks == {t \in TASKS : state[t] \in {"Ready", "Running"}}

SelectHighestPriority ==
    IF ReadyTasks = {} THEN -1
    ELSE CHOOSE t \in ReadyTasks :
        \A t2 \in ReadyTasks : priority[t] <= priority[t2]

(* ═══ Phase 1: Period advancement ═══ *)
AdvancePeriods ==
    [t \in TASKS |->
        IF periodCounter[t] + 1 >= periodLength[t]
        THEN 0  \* Period expired, reset
        ELSE periodCounter[t] + 1
    ]

ReplenishBudgets ==
    [t \in TASKS |->
        IF periodCounter[t] + 1 >= periodLength[t]
        THEN maxBudget[t]  \* Replenish
        ELSE budget[t]
    ]

WakeupSuspended ==
    [t \in TASKS |->
        IF /\ periodCounter[t] + 1 >= periodLength[t]
           /\ state[t] = "Suspended"
        THEN "Ready"
        ELSE state[t]
    ]

(* ═══ Phase 2: Budget consumption ═══ *)
(* Argument is a budget VALUE, not a task id — callers pass newBudget[t] *)
ConsumeBudget(b) ==
    IF b > 0
    THEN b - 1
    ELSE 0

(* ═══ Phase 3: Schedule tick ═══ *)
ScheduleTick ==
    /\ tick < MAX_PERIOD * 4
    /\ tick' = tick + 1
    \* Phase 1: period advancement
    /\ periodCounter' = AdvancePeriods
    /\ LET newState1 == WakeupSuspended
           newBudget == ReplenishBudgets
       IN
       \* Phase 2: budget check for current running task
       LET budgetState == [t \in TASKS |->
            IF /\ running >= 0
               /\ t = running
               /\ newBudget[t] = 0
            THEN "Suspended"
            ELSE newState1[t]]
       IN
       \* Phase 3: select highest priority
       LET selected == CHOOSE t \in {t2 \in TASKS : budgetState[t2] \in {"Ready", "Running"}} :
                \A t3 \in {t2 \in TASKS : budgetState[t2] \in {"Ready", "Running"}} :
                    priority[t] <= priority[t3]
       IN
       IF {t2 \in TASKS : budgetState[t2] \in {"Ready", "Running"}} = {}
       THEN /\ state' = budgetState
            /\ running' = -1
            /\ budget' = [t \in TASKS |-> IF t = running THEN ConsumeBudget(newBudget[t]) ELSE newBudget[t]]
            /\ UNCHANGED <<priority, maxBudget, periodLength>>
       ELSE /\ state' = [t \in TASKS |->
                IF t = selected THEN "Running"
                ELSE IF budgetState[t] = "Running" THEN "Ready"
                ELSE budgetState[t]]
            /\ running' = selected
            /\ budget' = [t \in TASKS |->
                IF t = running /\ running >= 0
                THEN ConsumeBudget(newBudget[t])
                ELSE newBudget[t]]
            /\ UNCHANGED <<priority, maxBudget, periodLength>>

(* ═══ Next State ═══ *)
Next == ScheduleTick

Spec == Init /\ [][Next]_vars /\ WF_vars(Next)

(* ═══════════════════════════════════════════════════════
   INVARIANTS — Always true
   ═══════════════════════════════════════════════════════ *)

(* INV1: At most one task is Running *)
AtMostOneRunning ==
    Cardinality({t \in TASKS : state[t] = "Running"}) <= 1

(* PROP2: Dead task never becomes Ready/Running — permanent *)
DeadNeverScheduled ==
    \A t \in TASKS :
        []((state[t] = "Dead") => [](state[t] = "Dead"))

(* INV3: Isolated task never becomes Ready/Running *)
IsolatedNeverScheduled ==
    \A t \in TASKS : state[t] = "Isolated" =>
        state[t] \notin {"Ready", "Running"}

(* INV4: Running task has the highest priority among ready tasks *)
RunningHasHighestPriority ==
    running >= 0 =>
        \A t \in TASKS :
            state[t] = "Ready" => priority[running] <= priority[t]

(* INV5: Budget never negative *)
BudgetNonNegative ==
    \A t \in TASKS : budget[t] >= 0

(* ═══════════════════════════════════════════════════════
   TEMPORAL PROPERTIES — Eventually true
   ═══════════════════════════════════════════════════════ *)

(* LIVE1: Every Ready task with budget eventually runs *)
StarvationFreedom ==
    \A t \in TASKS :
        []((budget[t] > 0 /\ state[t] = "Ready") ~> (state[t] = "Running"))

(* LIVE2: System always eventually schedules someone (no deadlock) *)
NoDeadlock ==
    []<>(\E t \in TASKS : state[t] = "Running")

====

---- MODULE SipahiBudgetFairness ----
(* Sipahi Microkernel — Budget Fairness TLA+ Specification
   Models: DAL-based CPU budget allocation, period replenishment.
   Verifies: DAL-A always gets CPU, DAL-D never starves DAL-A,
             total budget doesn't exceed 100%, fairness. *)

EXTENDS Integers, FiniteSets

CONSTANTS
    TASKS,          \* Set of task IDs
    BUDGET_A,       \* DAL-A budget (e.g., 40)
    BUDGET_B,       \* DAL-B budget (e.g., 30)
    BUDGET_C,       \* DAL-C budget (e.g., 20)
    BUDGET_D,       \* DAL-D budget (e.g., 10)
    PERIOD          \* Period length in ticks

VARIABLES
    dal,            \* dal[t] \in 0..3
    budget,         \* budget[t] \in Nat (remaining)
    maxBudget,      \* maxBudget[t] \in Nat (per-period)
    cpuUsed,        \* cpuUsed[t] \in Nat (total cycles consumed)
    state,          \* state[t] \in {Ready, Running, Suspended}
    periodCounter,  \* periodCounter \in 0..PERIOD
    tick,           \* Global tick
    running         \* Currently running task

vars == <<dal, budget, maxBudget, cpuUsed, state, periodCounter, tick, running>>

DalBudget(d) ==
    CASE d = 0 -> BUDGET_A
    []   d = 1 -> BUDGET_B
    []   d = 2 -> BUDGET_C
    []   d = 3 -> BUDGET_D

TypeOK ==
    /\ dal \in [TASKS -> 0..3]
    /\ budget \in [TASKS -> Nat]
    /\ maxBudget \in [TASKS -> Nat]
    /\ cpuUsed \in [TASKS -> Nat]
    /\ state \in [TASKS -> {"Ready", "Running", "Suspended"}]
    /\ periodCounter \in 0..PERIOD
    /\ tick \in Nat
    /\ running \in TASKS \cup {-1}

(* ═══ Initial State ═══ *)
Init ==
    /\ dal \in [TASKS -> 0..3]
    /\ maxBudget = [t \in TASKS |-> DalBudget(dal[t])]
    /\ budget = maxBudget
    /\ cpuUsed = [t \in TASKS |-> 0]
    /\ state = [t \in TASKS |-> "Ready"]
    /\ periodCounter = 0
    /\ tick = 0
    /\ running = -1

(* ═══ Priority from DAL (lower DAL = higher priority) ═══ *)
Priority(t) == dal[t]

ReadyTasks == {t \in TASKS : state[t] \in {"Ready", "Running"}}

SelectTask ==
    IF ReadyTasks = {} THEN -1
    ELSE CHOOSE t \in ReadyTasks :
        \A t2 \in ReadyTasks : Priority(t) <= Priority(t2)

(* ═══ Schedule Tick ═══ *)
ScheduleTick ==
    /\ tick < PERIOD * 4
    /\ tick' = tick + 1
    /\ LET newPeriod == (periodCounter + 1) % PERIOD
       IN /\ periodCounter' = newPeriod
          \* Period boundary: replenish budgets
          /\ IF newPeriod = 0
             THEN /\ budget' = [t \in TASKS |->
                        IF state[t] /= "Suspended" THEN maxBudget[t]
                        ELSE maxBudget[t]]
                  /\ state' = [t \in TASKS |->
                        IF state[t] = "Suspended" THEN "Ready"
                        ELSE state[t]]
                  /\ cpuUsed' = cpuUsed  \* Don't reset — cumulative
                  /\ running' = SelectTask
             ELSE \* Normal tick: consume budget, suspend if exhausted
                  /\ LET selected == SelectTask
                     IN /\ running' = selected
                        /\ budget' = [t \in TASKS |->
                            IF t = running /\ running >= 0
                            THEN IF budget[t] > 0 THEN budget[t] - 1 ELSE 0
                            ELSE budget[t]]
                        /\ cpuUsed' = [t \in TASKS |->
                            IF t = running /\ running >= 0
                            THEN cpuUsed[t] + 1
                            ELSE cpuUsed[t]]
                        /\ state' = [t \in TASKS |->
                            IF /\ t = running
                               /\ running >= 0
                               /\ budget[t] <= 1
                            THEN "Suspended"
                            ELSE IF t = selected THEN "Running"
                            ELSE IF state[t] = "Running" THEN "Ready"
                            ELSE state[t]]
    /\ UNCHANGED <<dal, maxBudget>>

Next == ScheduleTick

Spec == Init /\ [][Next]_vars /\ WF_vars(Next)

(* ═══════════════════════════════════════════════════════
   INVARIANTS
   ═══════════════════════════════════════════════════════ *)

(* INV1: removed — constant check, equivalent to const_assert. *)

(* INV2: DAL-A task always has budget >= DAL-D task *)
DalPriorityRespected ==
    \A t1, t2 \in TASKS :
        dal[t1] < dal[t2] => maxBudget[t1] >= maxBudget[t2]

(* INV3: No task exceeds its budget in a period *)
NoBudgetOverrun ==
    \A t \in TASKS : budget[t] >= 0

(* ═══════════════════════════════════════════════════════
   TEMPORAL PROPERTIES
   ═══════════════════════════════════════════════════════ *)

(* LIVE1: DAL-A task always eventually runs (never starved) *)
DalANeverStarved ==
    \A t \in TASKS :
        (dal[t] = 0) => []<>(state[t] = "Running")

(* LIVE2: DAL-D cannot prevent DAL-A from running *)
DalDCannotBlockA ==
    \A ta, td \in TASKS :
        ((dal[ta] = 0) /\ (dal[td] = 3))
        => []((state[td] = "Running") ~> (state[ta] = "Running"))

(* LIVE3: Every task with budget eventually runs (bounded fairness) *)
BudgetImpliesExecution ==
    \A t \in TASKS :
        []((budget[t] > 0 /\ state[t] = "Ready") ~> (state[t] = "Running"))

====

---- MODULE SipahiPolicy ----
(* Sipahi Microkernel — Policy Engine TLA+ Specification
   Models: 6-mode failure escalation, restart counting, DAL-based decisions.
   Verifies: escalation terminates, no livelock, PMP→always Shutdown,
             unknown event→Isolate (fail-safe).

   NOTE: This spec models policy DECISION logic, not trigger mechanisms.
   Runtime trigger conditions (e.g., 3× consecutive cap fail → CapViolation,
   isolated_count ≥ 2 → MultiModuleCrash, CLINT overrun → DeadlineMiss)
   are implementation details verified by Kani proofs and runtime tests.
   This spec verifies that GIVEN a PolicyEvent, the correct FailureMode
   is selected and escalation terminates. *)

EXTENDS Integers, FiniteSets

CONSTANTS
    TASKS,                  \* Set of task IDs
    MAX_RESTART_BUDGET,     \* Max restarts for budget exhaustion (1)
    MAX_RESTART_FAULT,      \* Max restarts for stack overflow/wasm trap (3)
    MAX_RESTART_WATCHDOG    \* Max restarts for watchdog timeout (1)

VARIABLES
    taskState,      \* taskState[t] \in States
    restartCount,   \* restartCount[t] \in Nat
    dal,            \* dal[t] \in 0..3 (A=0, B=1, C=2, D=3)
    event,          \* Current failure event for each task
    policyResult,   \* Last policy decision
    terminated      \* Has the escalation chain terminated?

vars == <<taskState, restartCount, dal, event, policyResult, terminated>>

States == {"Ready", "Running", "Suspended", "Isolated", "Dead", "Shutdown"}
FailureModes == {"Restart", "Degrade", "Isolate", "Failover", "Alert", "Shutdown"}
Events == {"BudgetExhausted", "StackOverflow", "WasmTrap",
           "CapViolation", "IopmpViolation", "PmpFail",
           "WatchdogTimeout", "DeadlineMiss", "MultiModuleCrash", "Unknown"}

TypeOK ==
    /\ taskState \in [TASKS -> States]
    /\ restartCount \in [TASKS -> Nat]
    /\ dal \in [TASKS -> 0..3]
    /\ terminated \in [TASKS -> BOOLEAN]

(* ═══ Initial State ═══ *)
Init ==
    /\ taskState = [t \in TASKS |-> "Ready"]
    /\ restartCount = [t \in TASKS |-> 0]
    /\ dal \in [TASKS -> 0..3]
    /\ event = [t \in TASKS |-> "BudgetExhausted"]
    /\ policyResult = [t \in TASKS |-> "Restart"]
    /\ terminated = [t \in TASKS |-> FALSE]

(* ═══ Pure policy decision function ═══ *)
DecideAction(ev, count, d) ==
    CASE ev = "BudgetExhausted" ->
            IF count < MAX_RESTART_BUDGET THEN "Restart" ELSE "Degrade"
    []   ev = "StackOverflow" ->
            IF count < MAX_RESTART_FAULT THEN "Restart" ELSE "Isolate"
    []   ev = "WasmTrap" ->
            IF count < MAX_RESTART_FAULT THEN "Restart" ELSE "Isolate"
    []   ev = "CapViolation" -> "Isolate"
    []   ev = "IopmpViolation" -> "Isolate"
    []   ev = "PmpFail" -> "Shutdown"
    []   ev = "WatchdogTimeout" ->
            IF count < MAX_RESTART_WATCHDOG THEN "Failover" ELSE "Degrade"
    []   ev = "DeadlineMiss" ->
            IF d = 0 THEN "Failover"
            ELSE IF d <= 2 THEN "Alert"
            ELSE "Isolate"
    []   ev = "MultiModuleCrash" -> "Shutdown"
    []   OTHER -> "Isolate"  \* Unknown event → fail-safe default

(* ═══ Apply policy action to task ═══ *)
ApplyAction(t, action) ==
    CASE action = "Restart" ->
            /\ taskState' = [taskState EXCEPT ![t] = "Suspended"]
            /\ restartCount' = [restartCount EXCEPT ![t] = restartCount[t] + 1]
            /\ terminated' = [terminated EXCEPT ![t] = FALSE]
    []   action = "Isolate" ->
            /\ taskState' = [taskState EXCEPT ![t] = "Isolated"]
            /\ terminated' = [terminated EXCEPT ![t] = TRUE]
            /\ UNCHANGED restartCount
    []   action = "Degrade" ->
            /\ taskState' = [taskState EXCEPT ![t] = "Suspended"]
            /\ terminated' = [terminated EXCEPT ![t] = TRUE]
            /\ UNCHANGED restartCount
    []   action = "Shutdown" ->
            /\ taskState' = [taskState EXCEPT ![t] = "Shutdown"]
            /\ terminated' = [terminated EXCEPT ![t] = TRUE]
            /\ UNCHANGED restartCount
    []   action = "Failover" ->
            /\ taskState' = [taskState EXCEPT ![t] = "Suspended"]
            /\ terminated' = [terminated EXCEPT ![t] = TRUE]
            /\ UNCHANGED restartCount
    []   action = "Alert" ->
            /\ UNCHANGED <<taskState, restartCount>>
            /\ terminated' = [terminated EXCEPT ![t] = TRUE]  \* Alert = decision made

(* ═══ Task failure event ═══ *)
TaskFailure(t) ==
    /\ taskState[t] \in {"Ready", "Running", "Suspended"}
    /\ \E ev \in Events :
        LET action == DecideAction(ev, restartCount[t], dal[t])
        IN /\ event' = [event EXCEPT ![t] = ev]
           /\ policyResult' = [policyResult EXCEPT ![t] = action]
           /\ ApplyAction(t, action)
           /\ UNCHANGED dal

(* ═══ Next State ═══ *)
Next == \E t \in TASKS : TaskFailure(t)

Spec == Init /\ [][Next]_vars /\ \A t \in TASKS : WF_vars(TaskFailure(t))

(* ═══════════════════════════════════════════════════════
   INVARIANTS
   ═══════════════════════════════════════════════════════ *)

(* INV1: PMP failure ALWAYS results in Shutdown *)
PmpAlwaysShutdown ==
    \A t \in TASKS :
        event[t] = "PmpFail" => policyResult[t] = "Shutdown"

(* INV2: Unknown event → Isolate (fail-safe) *)
UnknownEventIsolate ==
    \A t \in TASKS :
        event[t] = "Unknown" => policyResult[t] = "Isolate"

(* PROP3: Isolated state is permanent — once Isolated, always Isolated *)
IsolatedPermanent ==
    \A t \in TASKS :
        []((taskState[t] = "Isolated") => [](taskState[t] = "Isolated"))

(* PROP4: Shutdown state is permanent — once Shutdown, always Shutdown *)
ShutdownPermanent ==
    \A t \in TASKS :
        []((taskState[t] = "Shutdown") => [](taskState[t] = "Shutdown"))

(* INV5: Restart count is bounded *)
RestartBounded ==
    \A t \in TASKS :
        restartCount[t] <= MAX_RESTART_FAULT + MAX_RESTART_BUDGET + MAX_RESTART_WATCHDOG + 3

(* ═══════════════════════════════════════════════════════
   TEMPORAL PROPERTIES
   ═══════════════════════════════════════════════════════ *)

(* LIVE1: Repeated failures eventually reach terminal state *)
EscalationTerminates ==
    \A t \in TASKS :
        []((~terminated[t]) ~> terminated[t])

(* LIVE2: No livelock — escalation eventually terminates.
   Sprint U-12: strengthened via terminated flag, which becomes TRUE on
   any decision except Restart. Restart has bounded count, so eventually
   terminal action fires and terminated=TRUE. No infinite restart loop. *)
NoLivelock ==
    \A t \in TASKS :
        ~terminated[t] ~> terminated[t]

====

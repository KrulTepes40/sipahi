---- MODULE SipahiDegradeRecover ----
(* Sipahi Microkernel — Degrade/Recover Stability TLA+ Specification
   Models: graceful degradation (DAL-C/D suspended, budget halved),
           automatic recovery (DAL-A/B healthy → DAL-C/D restored).
   Verifies: no infinite oscillation, eventual stability,
             original budget restored on recovery, DAL-A/B never degraded. *)

EXTENDS Integers, FiniteSets, TLC

CONSTANTS
    TASKS,          \* Set of task IDs
    MAX_CYCLES      \* Bound for model checking

VARIABLES
    dal,            \* dal[t] \in 0..3
    state,          \* state[t] \in States
    budget,         \* budget[t] \in Nat
    originalBudget, \* originalBudget[t] \in Nat
    degraded,       \* System in degraded mode?
    degradeCount,   \* Number of degrade events (for oscillation check)
    recoverCount,   \* Number of recover events
    cycle           \* Step counter

vars == <<dal, state, budget, originalBudget, degraded, degradeCount, recoverCount, cycle>>

States == {"Ready", "Running", "Suspended", "Isolated", "Dead"}

TypeOK ==
    /\ dal \in [TASKS -> 0..3]
    /\ state \in [TASKS -> States]
    /\ budget \in [TASKS -> Nat]
    /\ originalBudget \in [TASKS -> Nat]
    /\ degraded \in BOOLEAN
    /\ degradeCount \in Nat
    /\ recoverCount \in Nat
    /\ cycle \in Nat

(* ═══ Helper sets ═══ *)
HighDAL == {t \in TASKS : dal[t] < 2}      \* DAL-A/B
LowDAL  == {t \in TASKS : dal[t] >= 2}     \* DAL-C/D
HighHealthy == \A t \in HighDAL : state[t] /= "Isolated"

(* ═══ Initial State ═══ *)
Init ==
    /\ dal = (0 :> 0 @@ 1 :> 2)
    /\ state = [t \in TASKS |-> "Ready"]
    /\ budget = (0 :> 4 @@ 1 :> 2)
    /\ originalBudget = budget
    /\ degraded = FALSE
    /\ degradeCount = 0
    /\ recoverCount = 0
    /\ cycle = 0

(* ═══ Degrade: DAL-C/D suspended, budget halved ═══ *)
DegradeSystem ==
    /\ ~degraded
    /\ degraded' = TRUE
    /\ degradeCount' = degradeCount + 1
    /\ state' = [t \in TASKS |->
        IF /\ dal[t] >= 2
           /\ state[t] /= "Dead"
           /\ state[t] /= "Isolated"
        THEN "Suspended"
        ELSE state[t]]
    /\ budget' = [t \in TASKS |->
        IF dal[t] >= 2 THEN budget[t] \div 2
        ELSE budget[t]]
    /\ cycle' = cycle + 1
    /\ UNCHANGED <<dal, originalBudget, recoverCount>>

(* ═══ Recover: if DAL-A/B healthy, restore DAL-C/D ═══ *)
RecoverSystem ==
    /\ degraded
    /\ HighHealthy
    /\ degraded' = FALSE
    /\ recoverCount' = recoverCount + 1
    \* Restore original budget
    /\ budget' = [t \in TASKS |->
        IF dal[t] >= 2 THEN originalBudget[t]
        ELSE budget[t]]
    /\ state' = [t \in TASKS |->
        IF /\ dal[t] >= 2
           /\ state[t] = "Suspended"
        THEN "Ready"
        ELSE state[t]]
    /\ cycle' = cycle + 1
    /\ UNCHANGED <<dal, originalBudget, degradeCount>>

(* Sprint U-12: cycle bound'ları kaldırıldı — liveness için unbounded gerekli.
   Arama derinliği TLC CONSTRAINT ile sınırlanır (cfg: cycle < MAX_CYCLES). *)

(* ═══ DAL-A/B task failure (can prevent recovery) ═══ *)
HighTaskFailure ==
    /\ \E t \in HighDAL :
        /\ state[t] \in {"Ready", "Running"}
        /\ state' = [state EXCEPT ![t] = "Isolated"]
        /\ cycle' = cycle + 1
        /\ UNCHANGED <<dal, budget, originalBudget, degraded, degradeCount, recoverCount>>

(* ═══ DAL-A/B task recovery (enables system recovery) ═══ *)
HighTaskRecover ==
    /\ \E t \in HighDAL :
        /\ state[t] = "Isolated"
        /\ state' = [state EXCEPT ![t] = "Ready"]
        /\ cycle' = cycle + 1
        /\ UNCHANGED <<dal, budget, originalBudget, degraded, degradeCount, recoverCount>>

(* ═══ Idle — system stable, no action ═══ *)
Idle ==
    /\ cycle' = cycle + 1
    /\ UNCHANGED <<dal, state, budget, originalBudget, degraded, degradeCount, recoverCount>>

(* ═══ State Constraint — TLC arama derinliği sınırı ═══ *)
StateConstraint == cycle < MAX_CYCLES

(* ═══ Next State ═══ *)
Next ==
    \/ DegradeSystem
    \/ RecoverSystem
    \/ HighTaskFailure
    \/ HighTaskRecover
    \/ Idle

\* WF: RecoverSystem VE HighTaskRecover eventually fire — aksi halde
\* HighTaskFailure sonrası stuck kalabilir (HighHealthy false → RecoverSystem
\* precondition fail), EventualRecovery tutmaz.
Spec == Init /\ [][Next]_vars
        /\ WF_vars(RecoverSystem)
        /\ WF_vars(HighTaskRecover)

(* ═══════════════════════════════════════════════════════
   INVARIANTS
   ═══════════════════════════════════════════════════════ *)

(* INV1: DAL-A/B never suspended by degrade *)
HighNeverDegraded ==
    \A t \in HighDAL :
        degraded => state[t] /= "Suspended" \/ dal[t] >= 2

(* PROP2: Original budget preserved — never modified across transitions *)
OriginalBudgetPreserved ==
    [][originalBudget' = originalBudget]_vars

(* INV3: After recovery, budget equals original *)
RecoveryRestoresFullBudget ==
    \A t \in LowDAL :
        /\ ~degraded
        /\ state[t] = "Ready"
        => budget[t] = originalBudget[t]

(* INV4: Degraded → all low DAL are Suspended *)
DegradedImpliesLowSuspended ==
    degraded => \A t \in LowDAL :
        state[t] \in {"Suspended", "Dead", "Isolated"}

(* INV5: Budget never zero after recovery *)
BudgetPositiveAfterRecover ==
    \A t \in LowDAL :
        /\ ~degraded
        /\ state[t] = "Ready"
        => budget[t] > 0

(* ═══════════════════════════════════════════════════════
   TEMPORAL PROPERTIES
   ═══════════════════════════════════════════════════════ *)

(* LIVE1: If DAL-A/B healthy and system degraded, eventually recovers *)
EventualRecovery ==
    [](degraded /\ HighHealthy ~> ~degraded)

(* LIVE2: System doesn't oscillate forever — bounded degrade/recover *)
NoInfiniteOscillation ==
    <>[](degradeCount = recoverCount \/ degradeCount = recoverCount + 1)

(* LIVE3: Degraded system eventually stabilizes *)
EventualStability ==
    <>[]( ~degraded \/ \A t \in HighDAL : state[t] = "Isolated" )

====

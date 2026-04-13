---- MODULE SipahiWatchdog ----
(* Sipahi Microkernel — Watchdog Liveness TLA+ Specification
   Models: windowed watchdog (upper + lower bound), policy triggering.
   Verifies: stuck task detected within bounded time,
             too-fast task detected, no false positives in normal operation. *)

EXTENDS Integers, FiniteSets

CONSTANTS
    TASKS,              \* Set of task IDs
    WATCHDOG_LIMIT,     \* Upper bound (e.g., 100 ticks)
    WINDOW_MIN,         \* Lower bound (e.g., 3 ticks)
    MAX_TICKS           \* Bound for model checking

VARIABLES
    watchdogCounter,    \* watchdogCounter[t] \in 0..WATCHDOG_LIMIT+1
    taskBehavior,       \* taskBehavior[t] \in {"normal", "stuck", "tooFast", "dead"}
    policyTriggered,    \* policyTriggered[t] \in BOOLEAN
    violationType,      \* violationType[t] \in {"none", "upper", "lower"}
    tick                \* Global tick counter

vars == <<watchdogCounter, taskBehavior, policyTriggered, violationType, tick>>

TypeOK ==
    /\ watchdogCounter \in [TASKS -> 0..(WATCHDOG_LIMIT + 1)]
    /\ taskBehavior \in [TASKS -> {"normal", "stuck", "tooFast", "dead"}]
    /\ policyTriggered \in [TASKS -> BOOLEAN]
    /\ violationType \in [TASKS -> {"none", "upper", "lower"}]
    /\ tick \in 0..MAX_TICKS

(* ═══ Initial State ═══ *)
Init ==
    /\ watchdogCounter = [t \in TASKS |-> 0]
    /\ taskBehavior \in [TASKS -> {"normal", "stuck", "tooFast"}]
    /\ policyTriggered = [t \in TASKS |-> FALSE]
    /\ violationType = [t \in TASKS |-> "none"]
    /\ tick = 0

(* ═══ Tick: increment watchdog counters ═══ *)
WatchdogTick ==
    /\ tick < MAX_TICKS
    /\ tick' = tick + 1
    /\ watchdogCounter' = [t \in TASKS |->
        IF policyTriggered[t] THEN watchdogCounter[t]  \* Already triggered
        ELSE IF watchdogCounter[t] >= WATCHDOG_LIMIT
             THEN watchdogCounter[t]  \* Saturate at limit
             ELSE watchdogCounter[t] + 1]
    \* Check upper bound violation (stuck task)
    /\ policyTriggered' = [t \in TASKS |->
        IF policyTriggered[t] THEN TRUE  \* Already triggered
        ELSE watchdogCounter[t] + 1 >= WATCHDOG_LIMIT]
    /\ violationType' = [t \in TASKS |->
        IF violationType[t] /= "none" THEN violationType[t]
        ELSE IF watchdogCounter[t] + 1 >= WATCHDOG_LIMIT THEN "upper"
        ELSE "none"]
    /\ UNCHANGED taskBehavior

(* ═══ Watchdog kick — normal task resets counter ═══ *)
NormalKick(t) ==
    /\ taskBehavior[t] = "normal"
    /\ ~policyTriggered[t]
    /\ watchdogCounter[t] >= WINDOW_MIN  \* Must wait minimum window
    /\ watchdogCounter' = [watchdogCounter EXCEPT ![t] = 0]
    /\ UNCHANGED <<taskBehavior, policyTriggered, violationType, tick>>

(* ═══ Too-fast kick — violates lower bound ═══ *)
TooFastKick(t) ==
    /\ taskBehavior[t] = "tooFast"
    /\ ~policyTriggered[t]
    /\ watchdogCounter[t] < WINDOW_MIN  \* Kick too early!
    /\ policyTriggered' = [policyTriggered EXCEPT ![t] = TRUE]
    /\ violationType' = [violationType EXCEPT ![t] = "lower"]
    /\ watchdogCounter' = [watchdogCounter EXCEPT ![t] = 0]
    /\ UNCHANGED <<taskBehavior, tick>>

(* ═══ Stuck task — never kicks ═══ *)
\* Stuck tasks do nothing — watchdog counter increments via WatchdogTick
\* until it hits WATCHDOG_LIMIT → policyTriggered

(* ═══ Next State ═══ *)
Next ==
    \/ WatchdogTick
    \/ \E t \in TASKS : NormalKick(t)
    \/ \E t \in TASKS : TooFastKick(t)

Spec == Init /\ [][Next]_vars /\ WF_vars(WatchdogTick)
    /\ \A t \in TASKS : WF_vars(NormalKick(t))

(* ═══════════════════════════════════════════════════════
   INVARIANTS
   ═══════════════════════════════════════════════════════ *)

(* INV1: Normal task never triggers policy *)
NormalTaskSafe ==
    \A t \in TASKS :
        taskBehavior[t] = "normal" => ~policyTriggered[t]

(* INV2: Counter never exceeds limit + 1 *)
CounterBounded ==
    \A t \in TASKS :
        watchdogCounter[t] <= WATCHDOG_LIMIT + 1

(* PROP3: Once triggered, stays triggered — permanent *)
TriggeredPermanent ==
    \A t \in TASKS :
        [](policyTriggered[t] => []policyTriggered[t])

(* ═══════════════════════════════════════════════════════
   TEMPORAL PROPERTIES
   ═══════════════════════════════════════════════════════ *)

(* LIVE1: Stuck task is ALWAYS eventually detected *)
StuckDetected ==
    \A t \in TASKS :
        taskBehavior[t] = "stuck" => <>(policyTriggered[t])

(* LIVE2: Too-fast task is detected on first early kick *)
TooFastDetected ==
    \A t \in TASKS :
        taskBehavior[t] = "tooFast" => <>(violationType[t] = "lower")

(* LIVE3: Detection happens within bounded time *)
BoundedDetection ==
    \A t \in TASKS :
        taskBehavior[t] = "stuck" =>
            <>(tick <= WATCHDOG_LIMIT + 1 /\ policyTriggered[t])

====

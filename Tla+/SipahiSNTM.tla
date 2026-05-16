---- MODULE SipahiSNTM ----
(* Sipahi Microkernel — SNTM Phase 3 TLA+ Specification
   Models: task lifecycle (Loaded/Ready/Running/Isolated/Dead),
           PMP profile reload atomicity (DENY stage + new profile + sfence).
   Verifies:
     - Kernel + UART PMP entries (0..7) NEVER change after boot (FIX-1)
     - U-mode never has interrupts enabled without an active task
     - At most one task running at any state
     - Isolated/Dead tasks never current

   SNTM design v0.8 §4.5.3 — Context Switch PMP Reload Atomicity invariant. *)

EXTENDS Integers, FiniteSets

CONSTANTS
    Tasks,                 \* Set of task IDs
    ReservedLowEntries,    \* Kernel + UART PMP entries (8)
    MaxPmpEntries          \* Total PMP entry budget (16)

ASSUME
    /\ Cardinality(Tasks) \in 1..4
    /\ ReservedLowEntries = 8
    /\ MaxPmpEntries = 16

VARIABLES
    state,           \* state[t] \in TaskState — task lifecycle
    pmp,             \* pmp[i] \in {"Locked", "Dynamic", "Off"} — entry classification
    kernelPmpInit,   \* Ghost: boot-time kernel+UART snapshot (immutable)
    miMode,          \* TRUE if M-mode, FALSE if U-mode
    mie,             \* mstatus.MIE interrupt-enable bit
    currentTask      \* Currently running task or "NONE"

vars == <<state, pmp, kernelPmpInit, miMode, mie, currentTask>>

TaskState == { "Loaded", "Ready", "Running", "Isolated", "Dead" }
PmpClass  == { "Locked", "Dynamic", "Off" }

Init ==
    /\ state = [t \in Tasks |-> "Loaded"]
    /\ pmp = [i \in 0..(MaxPmpEntries - 1) |->
              IF i < ReservedLowEntries THEN "Locked" ELSE "Off"]
    /\ kernelPmpInit = [i \in 0..(ReservedLowEntries - 1) |-> "Locked"]
    /\ miMode = TRUE        \* boot in M-mode
    /\ mie = FALSE          \* interrupts disabled at boot
    /\ currentTask = "NONE"

(* FIX-1: kernel + UART entries (0..ReservedLowEntries-1) NEVER change.
   ReloadAtomic only rewrites dynamic range (ReservedLowEntries..). *)
KernelEntriesUnchanged(newPmp) ==
    \A i \in 0..(ReservedLowEntries - 1) :
        newPmp[i] = kernelPmpInit[i]

(* SNTM design v0.8 §4.5.3 reload sequence — atomic M-mode + MIE=0 + DENY stage.
   Modeled abstractly: kernel entries unchanged, dynamic entries become "Dynamic"
   or "Off" — full combinatorial exploration would explode state space (3^8).
   Deterministic for model checking — atomicity invariant is what matters. *)
ReloadAtomic ==
    /\ miMode = TRUE
    /\ mie = FALSE
    /\ pmp' = [i \in 0..(MaxPmpEntries - 1) |->
              IF i < ReservedLowEntries
              THEN kernelPmpInit[i]
              ELSE "Dynamic"]

(* ─── Task lifecycle transitions ───────────────────────── *)

Boot(t) ==
    /\ state[t] = "Loaded"
    /\ state' = [state EXCEPT ![t] = "Ready"]
    /\ UNCHANGED <<pmp, kernelPmpInit, miMode, mie, currentTask>>

(* U-26 SNTM Phase 4 — Native task load transition.
   Boot context (M-mode, MIE=0). Loader bounded_copy + zero_fill yapar AMA
   pmp entry class flag'i (Locked/Dynamic/Off) bu adımda değişmez — kernel
   entries "Locked" kalır, dynamic entries hâlâ "Off" (Dispatch action
   onları "Dynamic" yapar). LoaderInvariant kernel range overwrite YOK
   garantisi. SNTM-R9 + SNTM-R10. *)
LoadNative(t) ==
    /\ state[t] = "Loaded"
    /\ miMode = TRUE
    /\ mie = FALSE
    /\ state' = [state EXCEPT ![t] = "Ready"]
    /\ UNCHANGED <<pmp, kernelPmpInit, miMode, mie, currentTask>>

Dispatch(t) ==
    /\ state[t] = "Ready"
    /\ currentTask = "NONE"
    /\ miMode = TRUE
    /\ mie = FALSE
    /\ ReloadAtomic
    /\ state' = [state EXCEPT ![t] = "Running"]
    /\ currentTask' = t
    /\ miMode' = FALSE          \* mret → U-mode
    /\ mie' = TRUE              \* mstatus.MIE bit set in mstatus
    /\ UNCHANGED kernelPmpInit

Preempt(t) ==
    /\ currentTask = t
    /\ state[t] = "Running"
    /\ state' = [state EXCEPT ![t] = "Ready"]
    /\ currentTask' = "NONE"
    /\ miMode' = TRUE           \* trap → M-mode
    /\ mie' = FALSE
    /\ UNCHANGED <<pmp, kernelPmpInit>>

ExitVoluntary(t) ==
    /\ currentTask = t
    /\ state[t] = "Running"
    /\ state' = [state EXCEPT ![t] = "Isolated"]
    /\ currentTask' = "NONE"
    /\ miMode' = TRUE
    /\ mie' = FALSE
    /\ UNCHANGED <<pmp, kernelPmpInit>>

Isolate(t) ==
    /\ state[t] \in {"Running", "Ready"}
    /\ state' = [state EXCEPT ![t] = "Isolated"]
    /\ currentTask' = IF currentTask = t THEN "NONE" ELSE currentTask
    /\ miMode' = IF currentTask = t THEN TRUE ELSE miMode
    /\ mie' = IF currentTask = t THEN FALSE ELSE mie
    /\ UNCHANGED <<pmp, kernelPmpInit>>

Next ==
    \/ \E t \in Tasks : Boot(t)
    \/ \E t \in Tasks : LoadNative(t)        \* U-26 SNTM Phase 4
    \/ \E t \in Tasks : Dispatch(t)
    \/ \E t \in Tasks : Preempt(t)
    \/ \E t \in Tasks : ExitVoluntary(t)
    \/ \E t \in Tasks : Isolate(t)

Spec == Init /\ [][Next]_vars

(* ─── INVARIANTS ─────────────────────────────────────────── *)

TypeOK ==
    /\ state \in [Tasks -> TaskState]
    /\ pmp \in [0..(MaxPmpEntries - 1) -> PmpClass]
    /\ kernelPmpInit \in [0..(ReservedLowEntries - 1) -> PmpClass]
    /\ miMode \in BOOLEAN
    /\ mie \in BOOLEAN
    /\ currentTask = "NONE" \/ currentTask \in Tasks

(* INV 1 (SNTM-R6 + FIX-1): Kernel + UART PMP entries bit-equal to boot snapshot.
   No reload path overwrites entries 0..ReservedLowEntries-1. *)
KernelPmpInvariant ==
    \A i \in 0..(ReservedLowEntries - 1) :
        pmp[i] = kernelPmpInit[i]

(* INV 2: U-mode requires an active task (no orphan U-mode execution). *)
UModeRequiresDispatch ==
    miMode = FALSE => currentTask /= "NONE"

(* INV 3: Isolated/Dead tasks never current. *)
NoIsolatedRunning ==
    \A t \in Tasks :
        state[t] \in {"Isolated", "Dead"} => currentTask /= t

(* INV 4: At most one task in Running state. *)
AtMostOneRunning ==
    \A t1, t2 \in Tasks :
        (state[t1] = "Running" /\ state[t2] = "Running") => t1 = t2

(* INV 5: Running task must equal currentTask. *)
RunningIsCurrent ==
    \A t \in Tasks :
        state[t] = "Running" => currentTask = t

(* INV 6 (U-26 SNTM-R9): LoadNative kernel range overwrite YOK.
   Loader transition sırasında kernel entries (0..ReservedLowEntries-1)
   "Locked" class flag'i değişmez — KernelPmpInvariant'ın TLA+ refinement
   katmanında loader-spesifik tekrarı (SNTM-R9 traceability). *)
LoaderInvariant ==
    \A i \in 0..(ReservedLowEntries - 1) :
        pmp[i] = kernelPmpInit[i]

THEOREM Spec => []TypeOK
THEOREM Spec => []KernelPmpInvariant
THEOREM Spec => []LoaderInvariant
THEOREM Spec => []UModeRequiresDispatch
THEOREM Spec => []NoIsolatedRunning
THEOREM Spec => []AtMostOneRunning
THEOREM Spec => []RunningIsCurrent

====

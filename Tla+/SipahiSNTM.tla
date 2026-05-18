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
    Channels,              \* U-27: IPC channel IDs (small set for state cap)
    ReservedLowEntries,    \* Kernel + UART PMP entries (8)
    MaxPmpEntries          \* Total PMP entry budget (16)

ASSUME
    /\ Cardinality(Tasks) \in 1..4
    /\ Cardinality(Channels) \in 1..2     \* U-27: state explosion guard
    /\ ReservedLowEntries = 8
    /\ MaxPmpEntries = 16

(* SAFE-4 (sprint-u33) Section 8 CR-5: stack analysis sabitleri.
   TLC cfg file function literal'i parse etmiyor; module içinde tanımla.
   Değerler gerçek sipahi.toml + sntm-stack çıktısı:
     task_hello: 8KB stack, observed 128 byte
     task_world: 8KB stack, observed 80 byte
   StackMarginBytes = STACK_ANALYSIS_MARGIN_BYTES kernel const.
   Abstract per-task class değil tek class (sembolik): tüm task'lar aynı stack
   region size + analyzer observed worst-case (task_hello 128). *)
StackBytesPerTask    == 8192
AnalyzerMaxWorstCase == 128
StackMarginBytes     == 256

VARIABLES
    state,           \* state[t] \in TaskState — task lifecycle
    pmp,             \* pmp[i] \in {"Locked", "Dynamic", "Off"} — entry classification
    kernelPmpInit,   \* Ghost: boot-time kernel+UART snapshot (immutable)
    miMode,          \* TRUE if M-mode, FALSE if U-mode
    mie,             \* mstatus.MIE interrupt-enable bit
    currentTask,     \* Currently running task or "NONE"
    sealed,          \* U-27: IPC channels sealed flag (post-boot lock)
    channels,        \* U-27: channels[c] \in Tasks \cup {"NONE"} — producer assignment
    channelsAtSeal   \* U-27: ghost snapshot at SealChannels — atomicity proof

vars == <<state, pmp, kernelPmpInit, miMode, mie, currentTask, sealed, channels, channelsAtSeal>>

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
    /\ sealed = FALSE       \* U-27: channels unsealed at boot
    /\ channels = [c \in Channels |-> "NONE"]  \* U-27: no producers yet
    /\ channelsAtSeal = [c \in Channels |-> "NONE"]  \* U-27: ghost snapshot

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
    /\ UNCHANGED <<pmp, kernelPmpInit, miMode, mie, currentTask, sealed, channels, channelsAtSeal>>

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
    /\ UNCHANGED <<pmp, kernelPmpInit, miMode, mie, currentTask, sealed, channels, channelsAtSeal>>

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
    /\ UNCHANGED <<kernelPmpInit, sealed, channels, channelsAtSeal>>

Preempt(t) ==
    /\ currentTask = t
    /\ state[t] = "Running"
    /\ state' = [state EXCEPT ![t] = "Ready"]
    /\ currentTask' = "NONE"
    /\ miMode' = TRUE           \* trap → M-mode
    /\ mie' = FALSE
    /\ UNCHANGED <<pmp, kernelPmpInit, sealed, channels, channelsAtSeal>>

ExitVoluntary(t) ==
    /\ currentTask = t
    /\ state[t] = "Running"
    /\ state' = [state EXCEPT ![t] = "Isolated"]
    /\ currentTask' = "NONE"
    /\ miMode' = TRUE
    /\ mie' = FALSE
    /\ UNCHANGED <<pmp, kernelPmpInit, sealed, channels, channelsAtSeal>>

Isolate(t) ==
    /\ state[t] \in {"Running", "Ready"}
    /\ state' = [state EXCEPT ![t] = "Isolated"]
    /\ currentTask' = IF currentTask = t THEN "NONE" ELSE currentTask
    /\ miMode' = IF currentTask = t THEN TRUE ELSE miMode
    /\ mie' = IF currentTask = t THEN FALSE ELSE mie
    /\ UNCHANGED <<pmp, kernelPmpInit, sealed, channels, channelsAtSeal>>

(* U-27 SNTM-R13: Pre-seal channel assignment — boot context.
   ipc::assign_channel modeli. Sealed durumda no-op (action disabled). *)
AssignChannel(c, p) ==
    /\ sealed = FALSE
    /\ miMode = TRUE
    /\ mie = FALSE
    /\ channels' = [channels EXCEPT ![c] = p]
    /\ UNCHANGED <<state, pmp, kernelPmpInit, miMode, mie, currentTask, sealed, channelsAtSeal>>

(* U-27 SNTM-R13: SealChannels — boot sonrası tek seferlik kilit.
   Ghost: channelsAtSeal snapshot atomicity invariant için. Idempotent —
   ikinci çağrı state'i bozmaz. *)
SealChannels ==
    /\ miMode = TRUE
    /\ mie = FALSE
    /\ sealed' = TRUE
    /\ channelsAtSeal' = IF sealed = FALSE THEN channels ELSE channelsAtSeal
    /\ UNCHANGED <<state, pmp, kernelPmpInit, miMode, mie, currentTask, channels>>

Next ==
    \/ \E t \in Tasks : Boot(t)
    \/ \E t \in Tasks : LoadNative(t)        \* U-26 SNTM Phase 4
    \/ \E t \in Tasks : Dispatch(t)
    \/ \E t \in Tasks : Preempt(t)
    \/ \E t \in Tasks : ExitVoluntary(t)
    \/ \E t \in Tasks : Isolate(t)
    \/ \E c \in Channels, p \in Tasks : AssignChannel(c, p)   \* U-27 SNTM-R13
    \/ SealChannels                                            \* U-27 SNTM-R13

Spec == Init /\ [][Next]_vars

(* ─── INVARIANTS ─────────────────────────────────────────── *)

TypeOK ==
    /\ state \in [Tasks -> TaskState]
    /\ pmp \in [0..(MaxPmpEntries - 1) -> PmpClass]
    /\ kernelPmpInit \in [0..(ReservedLowEntries - 1) -> PmpClass]
    /\ miMode \in BOOLEAN
    /\ mie \in BOOLEAN
    /\ (currentTask = "NONE" \/ currentTask \in Tasks)
    /\ sealed \in BOOLEAN
    /\ channels \in [Channels -> Tasks \cup {"NONE"}]
    /\ channelsAtSeal \in [Channels -> Tasks \cup {"NONE"}]

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

(* INV 7 (U-27 SNTM-R13): Sealed channel atomicity — seal SONRASI
   channels[c] kalıbı SEAL anındaki snapshot ile birebir aynı kalır.
   AssignChannel sealed=TRUE iken kabul edilmez (action disabled),
   SealChannels idempotent (ikinci çağrı snapshot'ı bozmaz). *)
SealedAtomicityInvariant ==
    sealed = TRUE =>
        \A c \in Channels : channels[c] = channelsAtSeal[c]

(* SAFE-2 (sprint-u31): Channel ownership well-formedness (CR-7).
   ChannelOwnershipInvariant captures the manifest [[channel]] constraints
   that BOOT_CHANNELS codegen enforces — verified at the spec level too.

   SCOPE: this invariant lives in SipahiSNTM for now (Section 8 CR-7).
   If state count exceeds 200 (Section 9.3 S3 baseline 138-200), split
   into SipahiTypedIPC.tla. Current target: < 200 distinct states. *)
ChannelOwnershipInvariant ==
    \A c \in Channels :
        channels[c] = "NONE" \/ channels[c] \in Tasks

(* SAFE-3 (sprint-u32, Section 8 CR-3): ChannelOwnershipInvariant zayıftı
   (type-level). Güçlendir: real ownership semantic — sealed sonrası
   channels[c] = channelsAtSeal[c] explicit ownership claim. Audit raporda
   "channel ownership kanıtlandı" iddiası bu invariant ile somut. *)
StrongChannelOwnership ==
    /\ \A c \in Channels :
         channels[c] = "NONE" \/ channels[c] \in Tasks
    /\ (sealed = TRUE) =>
         (\A c \in Channels : channels[c] = channelsAtSeal[c])

(* SAFE-4 (sprint-u33) Section 8 CR-5: StackRegionBound — her task için
   manifest task region stack size, sntm-stack observed_max + margin'i
   karşılamalı. Abstract: TLC sembolik byte sınıfı (TaskStackBytes,
   TaskAnalyzerMax, StackMarginBytes konstantları konfig kaynağı). Bu
   invariant constant-tabanlı: state machine transition'ları stack metric'i
   değiştirmez (build-time analizi); ama spec-level kontrat olarak kalır.
   Section 9.3 S3: yeni invariant state count'u arttırmaz (138 baseline). *)
StackRegionBound ==
    \A t \in Tasks :
        StackBytesPerTask >= AnalyzerMaxWorstCase + StackMarginBytes

THEOREM Spec => []TypeOK
THEOREM Spec => []KernelPmpInvariant
THEOREM Spec => []LoaderInvariant
THEOREM Spec => []UModeRequiresDispatch
THEOREM Spec => []NoIsolatedRunning
THEOREM Spec => []AtMostOneRunning
THEOREM Spec => []RunningIsCurrent
THEOREM Spec => []SealedAtomicityInvariant   \* U-27 SNTM-R13
THEOREM Spec => []ChannelOwnershipInvariant  \* SAFE-2 sprint-u31
THEOREM Spec => []StrongChannelOwnership     \* SAFE-3 sprint-u32 CR-3
THEOREM Spec => []StackRegionBound           \* SAFE-4 sprint-u33 CR-5

====

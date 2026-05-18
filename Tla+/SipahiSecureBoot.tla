---- MODULE SipahiSecureBoot ----
(* Sipahi SNTM-SAFE Image Boot — SAFE-3 (sprint-u32) §17.10 [10/10] gate.
   Models the image signature verification state machine performed at boot:
     Unverified → HeaderValid → SigValid → Booted
                                        ↘
                                          HaltedFail (on any tamper)

   Invariants:
     StartedImpliesValid: state = "Booted" => sig_valid
     NoFalseAccept:       tampered => sig_valid = FALSE
     AtomicVerify:        sig_valid değişimi atomic (intermediate yok)

   Liveness (state count guard, Section 9.3 S2):
     Spec eventually → "Booted" \/ "HaltedFail" — döngüde kalmaz. *)

EXTENDS Integers, FiniteSets

CONSTANTS
    Bytes  \* Symbolic input domain: NormalBytes ∪ TamperedBytes

ASSUME
    /\ Cardinality(Bytes) <= 4   \* state explosion guard

VARIABLES
    bootState,    \* State machine state
    sigValid,     \* Bool — signature verification result
    tampered      \* Bool — image was tampered (test only)

vars == <<bootState, sigValid, tampered>>

BootState == { "Unverified", "HeaderValid", "SigValid", "Booted", "HaltedFail" }

Init ==
    /\ bootState = "Unverified"
    /\ sigValid  = FALSE
    /\ tampered  \in BOOLEAN

(* Header check: magic + offsets parse — atomic transition. *)
VerifyHeader ==
    /\ bootState = "Unverified"
    /\ bootState' = IF tampered THEN "HaltedFail" ELSE "HeaderValid"
    /\ UNCHANGED <<sigValid, tampered>>

(* Signature verify: ed25519 over body. Tampered body → false. *)
VerifySignature ==
    /\ bootState = "HeaderValid"
    /\ sigValid' = ~tampered
    /\ bootState' = IF tampered THEN "HaltedFail" ELSE "SigValid"
    /\ UNCHANGED tampered

(* Kernel start: requires SigValid. *)
StartKernel ==
    /\ bootState = "SigValid"
    /\ bootState' = "Booted"
    /\ UNCHANGED <<sigValid, tampered>>

Next ==
    \/ VerifyHeader
    \/ VerifySignature
    \/ StartKernel

Spec == Init /\ [][Next]_vars /\ WF_vars(Next)

(* ─── INVARIANTS ─── *)

TypeOK ==
    /\ bootState \in BootState
    /\ sigValid  \in BOOLEAN
    /\ tampered  \in BOOLEAN

(* INV: Booted state IMPLIES sigValid TRUE — no false-accept. *)
StartedImpliesValid ==
    (bootState = "Booted") => (sigValid = TRUE)

(* INV: tampered image NEVER reaches Booted. *)
NoFalseAccept ==
    (tampered = TRUE) => (bootState # "Booted")

(* INV: sigValid only TRUE when state >= SigValid. *)
AtomicVerify ==
    (sigValid = TRUE) => (bootState \in {"SigValid", "Booted"})

(* INV (S5 negative simulation guard): SigValid implies header was passed. *)
SigValidImpliesHeader ==
    (bootState = "SigValid" \/ bootState = "Booted") =>
        (~tampered)

THEOREM Spec => []TypeOK
THEOREM Spec => []StartedImpliesValid
THEOREM Spec => []NoFalseAccept
THEOREM Spec => []AtomicVerify
THEOREM Spec => []SigValidImpliesHeader

====

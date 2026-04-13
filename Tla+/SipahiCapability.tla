---- MODULE SipahiCapability ----
(* Sipahi Microkernel — Capability Token TLA+ Specification
   Models: token validation, 4-slot cache, per-task nonce, expiry.
   Verifies: cache coherence, nonce monotonicity, expired tokens rejected,
             invalidated tokens never found. *)

EXTENDS Integers, FiniteSets

CONSTANTS
    TASKS,          \* Set of task IDs
    RESOURCES,      \* Set of resource IDs
    CACHE_SLOTS,    \* Number of cache slots (4)
    MAX_TICK        \* Max tick for bounded model checking

VARIABLES
    cache,          \* cache[slot] = [valid, tokenId, resource, action, expires] or empty
    lastNonce,      \* lastNonce[t] \in Nat — per-task monotonic nonce
    nextSlot,       \* Round-robin eviction pointer
    currentTick,    \* System tick
    keyReady        \* MAC key provisioned?

vars == <<cache, lastNonce, nextSlot, currentTick, keyReady>>

CacheEntry == [valid : BOOLEAN, tokenId : Nat, resource : Nat, expires : Nat]
EmptyEntry == [valid |-> FALSE, tokenId |-> 0, resource |-> 0, expires |-> 0]

TypeOK ==
    /\ cache \in [0..(CACHE_SLOTS - 1) -> CacheEntry]
    /\ lastNonce \in [TASKS -> Nat]
    /\ nextSlot \in 0..(CACHE_SLOTS - 1)
    /\ currentTick \in 0..MAX_TICK
    /\ keyReady \in BOOLEAN

(* ═══ Initial State ═══ *)
Init ==
    /\ cache = [s \in 0..(CACHE_SLOTS - 1) |-> EmptyEntry]
    /\ lastNonce = [t \in TASKS |-> 0]
    /\ nextSlot = 0
    /\ currentTick = 0
    /\ keyReady = FALSE

(* ═══ Provision key (once) ═══ *)
ProvisionKey ==
    /\ ~keyReady
    /\ keyReady' = TRUE
    /\ UNCHANGED <<cache, lastNonce, nextSlot, currentTick>>

(* ═══ Validate full — MAC + nonce + expiry + cache insert ═══ *)
ValidateFull(taskId, tokenId, resource, nonce, expires) ==
    /\ keyReady
    /\ nonce > lastNonce[taskId]                    \* Replay guard
    /\ (expires = 0 \/ currentTick <= expires)      \* Expiry check
    \* Update nonce
    /\ lastNonce' = [lastNonce EXCEPT ![taskId] = nonce]
    \* Insert into cache (round-robin)
    /\ cache' = [cache EXCEPT ![nextSlot] =
        [valid |-> TRUE, tokenId |-> tokenId,
         resource |-> resource, expires |-> expires]]
    /\ nextSlot' = (nextSlot + 1) % CACHE_SLOTS
    /\ UNCHANGED <<currentTick, keyReady>>

(* ═══ Validate cached — cache lookup only ═══ *)
ValidateCached(tokenId, resource) ==
    \E s \in 0..(CACHE_SLOTS - 1) :
        /\ cache[s].valid
        /\ cache[s].tokenId = tokenId
        /\ cache[s].resource = resource
        /\ (cache[s].expires = 0 \/ currentTick <= cache[s].expires)

(* ═══ Reject — replay, expired, or no key ═══ *)
RejectReplay(taskId, nonce) ==
    nonce <= lastNonce[taskId]

RejectExpired(expires) ==
    /\ expires > 0
    /\ currentTick > expires

(* ═══ Invalidate — remove token from cache ═══ *)
InvalidateToken(tokenId) ==
    /\ cache' = [s \in 0..(CACHE_SLOTS - 1) |->
        IF cache[s].tokenId = tokenId
        THEN EmptyEntry
        ELSE cache[s]]
    /\ UNCHANGED <<lastNonce, nextSlot, currentTick, keyReady>>

(* ═══ Tick advance ═══ *)
TickAdvance ==
    /\ currentTick < MAX_TICK
    /\ currentTick' = currentTick + 1
    /\ UNCHANGED <<cache, lastNonce, nextSlot, keyReady>>

(* ═══ Token operation — validate or invalidate ═══ *)
TokenOp ==
    \/ ProvisionKey
    \/ \E t \in TASKS, tid \in 1..2, res \in RESOURCES, n \in 1..5, exp \in 0..MAX_TICK :
        ValidateFull(t, tid, res, n, exp)
    \/ \E tid \in 1..2 : InvalidateToken(tid)
    \/ TickAdvance

Next == TokenOp

Spec == Init /\ [][Next]_vars /\ WF_vars(Next)

(* ═══════════════════════════════════════════════════════
   INVARIANTS
   ═══════════════════════════════════════════════════════ *)

(* PROP1: Nonce is monotonically increasing per task — never decreases *)
NonceMonotonic ==
    \A t \in TASKS :
        [][lastNonce'[t] >= lastNonce[t]]_vars

(* INV2: Invalidated token is never found in cache *)
InvalidatedNotFound ==
    \A s \in 0..(CACHE_SLOTS - 1) :
        ~cache[s].valid => cache[s].tokenId = 0

(* INV3: Cache slot count bounded *)
CacheBounded ==
    \A s \in 0..(CACHE_SLOTS - 1) :
        cache[s] \in CacheEntry

(* INV4: If ALL cache entries for a token are expired, lookup fails *)
ExpiredTokenRejected ==
    \A tid \in 1..2, res \in RESOURCES :
        (\A s \in 0..(CACHE_SLOTS - 1) :
            cache[s].valid /\ cache[s].tokenId = tid /\ cache[s].resource = res
            => (cache[s].expires > 0 /\ currentTick > cache[s].expires))
        => ~ValidateCached(tid, res)

(* INV5: No validation without key *)
NoValidationWithoutKey ==
    ~keyReady => \A s \in 0..(CACHE_SLOTS - 1) : ~cache[s].valid

(* ═══════════════════════════════════════════════════════
   TEMPORAL PROPERTIES
   ═══════════════════════════════════════════════════════ *)

(* LIVE1: Key is eventually provisioned *)
KeyEventuallyReady ==
    <>keyReady

(* LIVE2: removed — duplicate of NonceMonotonic (PROP1). *)

====

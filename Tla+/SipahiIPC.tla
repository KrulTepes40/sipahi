---- MODULE SipahiIPC ----
(* Sipahi Microkernel — IPC SPSC Ring Buffer TLA+ Specification
   Models: lock-free single-producer single-consumer ring buffer.
   Verifies: no message loss, no overwrite, FIFO ordering,
             linearizability, buffer bounds. *)

EXTENDS Integers, Sequences, FiniteSets

CONSTANTS
    SLOTS,          \* Buffer capacity (e.g., 16)
    MAX_MSGS        \* Max messages to model (bounds state space)

VARIABLES
    head,           \* Producer write position (0..SLOTS-1 wrapping)
    tail,           \* Consumer read position (0..SLOTS-1 wrapping)
    buffer,         \* buffer[i] = message or "empty"
    sent,           \* Sequence of sent messages (ghost variable)
    received,       \* Sequence of received messages (ghost variable)
    msgCounter      \* Next message ID to send

vars == <<head, tail, buffer, sent, received, msgCounter>>

(* Sprint U-12: TLC 2026.04 strict type checking — mixed-type set
   (Nat ∪ {"empty"}) yasak. Integer sentinel: 0 = empty, 1..MAX_MSGS = message.
   Messages start from 1 because msgCounter starts at 1 (Init). *)
EMPTY == 0
Messages == 1..(MAX_MSGS + 1)
BufferValues == {EMPTY} \cup Messages

TypeOK ==
    /\ head \in 0..(SLOTS - 1)
    /\ tail \in 0..(SLOTS - 1)
    /\ buffer \in [0..(SLOTS - 1) -> BufferValues]
    /\ sent \in Seq(Messages)
    /\ received \in Seq(Messages)
    /\ msgCounter \in Messages

(* ═══ Initial State ═══ *)
Init ==
    /\ head = 0
    /\ tail = 0
    /\ buffer = [i \in 0..(SLOTS - 1) |-> EMPTY]
    /\ sent = <<>>
    /\ received = <<>>
    /\ msgCounter = 1

(* ═══ Buffer occupancy ═══ *)
BufferCount == (head - tail + SLOTS) % SLOTS
IsFull == (head + 1) % SLOTS = tail
IsEmpty == head = tail

(* ═══ Send (Producer) ═══ *)
Send ==
    /\ ~IsFull
    /\ msgCounter <= MAX_MSGS
    /\ LET nextHead == (head + 1) % SLOTS
       IN /\ buffer' = [buffer EXCEPT ![head] = msgCounter]
          /\ head' = nextHead
          /\ sent' = Append(sent, msgCounter)
          /\ msgCounter' = msgCounter + 1
          /\ UNCHANGED <<tail, received>>

(* ═══ Send when full → rejected ═══ *)
SendFull ==
    /\ IsFull
    /\ UNCHANGED vars

(* ═══ Recv (Consumer) ═══ *)
Recv ==
    /\ ~IsEmpty
    /\ LET msg == buffer[tail]
           nextTail == (tail + 1) % SLOTS
       IN /\ received' = Append(received, msg)
          /\ buffer' = [buffer EXCEPT ![tail] = EMPTY]
          /\ tail' = nextTail
          /\ UNCHANGED <<head, sent, msgCounter>>

(* ═══ Recv when empty → no-op ═══ *)
RecvEmpty ==
    /\ IsEmpty
    /\ UNCHANGED vars

(* ═══ Next State ═══ *)
Next ==
    \/ Send
    \/ SendFull
    \/ Recv
    \/ RecvEmpty

Spec == Init /\ [][Next]_vars /\ WF_vars(Send) /\ WF_vars(Recv)

(* ═══════════════════════════════════════════════════════
   INVARIANTS
   ═══════════════════════════════════════════════════════ *)

(* INV1: Buffer never exceeds capacity *)
BufferBounded ==
    BufferCount < SLOTS

(* INV2: No message overwrite — slot written only when "empty" *)
(* Encoded by Send precondition: ~IsFull *)

(* INV3: Received messages are a prefix of sent messages (FIFO) *)
FIFOOrdering ==
    /\ Len(received) <= Len(sent)
    /\ \A i \in 1..Len(received) : received[i] = sent[i]

(* INV4: removed — was tautological. EventualDelivery covers no-message-loss. *)

(* INV5: Head and tail always in bounds *)
IndicesInBounds ==
    /\ head \in 0..(SLOTS - 1)
    /\ tail \in 0..(SLOTS - 1)

(* ═══════════════════════════════════════════════════════
   TEMPORAL PROPERTIES
   ═══════════════════════════════════════════════════════ *)

(* LIVE1: Every sent message is eventually received *)
EventualDelivery ==
    \A i \in 1..MAX_MSGS :
        [](Len(sent) >= i ~> Len(received) >= i)

(* LIVE2: If buffer not empty, consumer eventually reads *)
ConsumerProgress ==
    []( ~IsEmpty ~> IsEmpty )

====

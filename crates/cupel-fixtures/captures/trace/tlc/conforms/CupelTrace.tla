---- MODULE CupelTrace ----
EXTENDS Counter, Integers, Sequences, TLC

VARIABLE cupel_step

CupelTrace == <<
    [count |-> 0, done |-> FALSE],
    [count |-> 1, done |-> FALSE],
    [count |-> 2, done |-> FALSE],
    [count |-> 2, done |-> TRUE]
>>

CupelTraceAction == <<"", "Inc", "Inc", "Finish">>

CupelTraceInit ==
    /\ cupel_step = 1
    /\ Init
    /\ ("count" \in DOMAIN CupelTrace[1] => count = CupelTrace[1]["count"])
    /\ ("done" \in DOMAIN CupelTrace[1] => done = CupelTrace[1]["done"])

CupelTraceNext ==
    /\ cupel_step < Len(CupelTrace)
    /\ cupel_step' = cupel_step + 1
    /\ CASE CupelTraceAction[cupel_step + 1] = "Finish" -> Finish
         [] CupelTraceAction[cupel_step + 1] = "Inc" -> Inc
         [] OTHER -> Next
    /\ ("count" \in DOMAIN CupelTrace[cupel_step + 1] => count' = CupelTrace[cupel_step + 1]["count"])
    /\ ("done" \in DOMAIN CupelTrace[cupel_step + 1] => done' = CupelTrace[cupel_step + 1]["done"])

CupelTraceUnmatched == cupel_step < Len(CupelTrace)
====

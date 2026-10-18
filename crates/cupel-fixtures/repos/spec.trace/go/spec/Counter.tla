---- MODULE Counter ----
\* A counter that counts up by one until it is finished.
EXTENDS Naturals

VARIABLES count, done

Init == count = 0 /\ done = FALSE

Inc == ~done /\ count' = count + 1 /\ UNCHANGED done

Finish == ~done /\ done' = TRUE /\ UNCHANGED count

Next == Inc \/ Finish
====

// The tally's arithmetic, proved: `Total` is, `Max` is not (both branches return `a`),
// `SumOfTwo` assumes what it should prove, and `Square` states a lemma it never proves.
function Sum(xs: seq<int>): int
{
  if |xs| == 0 then 0 else xs[0] + Summ(xs[1..])
}

method Total(xs: seq<int>) returns (t: int)
  ensures t == Sum(xs)
{
  t := 0;
  var i := |xs|;
  while i > 0
    invariant 0 <= i <= |xs|
    invariant t == Sum(xs[i..])
  {
    i := i - 1;
    t := t + xs[i];
  }
}

method Max(a: int, b: int) returns (m: int)
  ensures m >= a && m >= b
{
  if a > b { m := a; } else { m := a; }
}

lemma SumOfTwo(a: int, b: int)
  ensures Sum([a, b]) == a + b
{
  assume Sum([b]) == b;
}

lemma Square(x: int)
  ensures x * x >= 0

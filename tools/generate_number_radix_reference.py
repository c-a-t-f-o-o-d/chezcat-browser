"""Freeze original exact-rational binary64 radix witnesses; no browser/engine data."""
from fractions import Fraction
from pathlib import Path
import hashlib
import math
import struct

ROOT = Path(__file__).resolve().parents[1]
OUT = ROOT / "tools/reference/number-radix-056/expected.tsv"
ALPHABET = "0123456789abcdefghijklmnopqrstuvwxyz"

def value(bits):
    return struct.unpack(">d", bits.to_bytes(8, "big"))[0]

def power(radix, exponent):
    return Fraction(radix ** exponent) if exponent >= 0 else Fraction(1, radix ** -exponent)

def reference(bits, radix):
    negative = bool(bits >> 63)
    bits &= (1 << 63) - 1
    x = Fraction.from_float(value(bits))
    previous = Fraction.from_float(value(bits - 1))
    following = Fraction.from_float(value(bits + 1)) if bits < 0x7fefffffffffffff else x + (x - previous)
    lower, upper = (previous + x) / 2, (x + following) / 2
    inclusive = bits % 2 == 0
    # A floating estimate only chooses a starting exponent. Exact comparisons
    # correct it, including values adjacent to integral radix powers.
    point = math.floor(math.log(value(bits)) / math.log(radix)) + 1
    while x < power(radix, point - 1): point -= 1
    while x >= power(radix, point): point += 1
    for digits in range(1, 129):
        exponent = point - digits
        scale = power(radix, exponent)
        lo, hi = lower / scale, upper / scale
        minimum = -(-lo.numerator // lo.denominator)
        maximum = hi.numerator // hi.denominator
        if not inclusive and lo.denominator == 1: minimum += 1
        if not inclusive and hi.denominator == 1: maximum -= 1
        if minimum > maximum: continue
        scaled = x / scale
        whole, remainder = divmod(scaled.numerator, scaled.denominator)
        nearest = whole + int(2 * remainder > scaled.denominator or
                              2 * remainder == scaled.denominator and whole % 2 != 0)
        coefficient = min(max(nearest, minimum), maximum)
        assert coefficient > 0
        chosen = coefficient * scale
        assert lower < chosen < upper or inclusive and chosen in (lower, upper)
        # Every coarser coefficient grid was tested before this first success.
        while coefficient % radix == 0:
            coefficient //= radix
            exponent += 1
        text = ""
        while coefficient:
            coefficient, digit = divmod(coefficient, radix)
            text = ALPHABET[digit] + text
        if exponent >= 0: text += "0" * exponent
        else:
            position = len(text) + exponent
            text = text[:position] + "." + text[position:] if position > 0 else "0." + "0" * -position + text
        return ("-" if negative else "") + text
    raise AssertionError("binary64 shortest witness exceeded bounded digits")

def main():
    cases = {1, 2, 3, 0x000ffffffffffffe, 0x000fffffffffffff, 0x0010000000000000,
             0x0010000000000001, 0x3fefffffffffffff, 0x3ff0000000000000, 0x3ff0000000000001,
             0x7feffffffffffffe, 0x7fefffffffffffff}
    for number in [0.1, 0.2, 1/3, 0.5, 0.125, 10.5, 35.0, 36.0, 255.0, 1e21, 1e100,
                   float(2**53-1), float(2**53), float(2**53+2), float(2**100)]:
        bits = int.from_bytes(struct.pack(">d", number), "big")
        cases.update([bits, bits | (1 << 63)])
    state = 0x56a1c9de4827b301
    for _ in range(256):
        state = (state * 6364136223846793005 + 1442695040888963407) & ((1 << 64) - 1)
        if (state >> 52) & 2047 != 2047 and state & ((1 << 63) - 1): cases.add(state)
    lines = ["# bits(hex)\tradix\texact shortest/nearest-even positional result"]
    for bits in sorted(cases):
        for radix in range(2, 37):
            if radix != 10: lines.append(f"{bits:016x}\t{radix}\t{reference(bits, radix)}")
    OUT.parent.mkdir(parents=True, exist_ok=True)
    OUT.write_bytes(("\n".join(lines) + "\n").encode("ascii"))
    print(f"{len(cases)} binary64 values; {len(lines)-1} exact interval/minimality witnesses; sha256={hashlib.sha256(OUT.read_bytes()).hexdigest()}")

if __name__ == "__main__": main()

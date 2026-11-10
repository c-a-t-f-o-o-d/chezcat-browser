// Original examples for ChezCat's binary and collection runtime.
var memory = new ArrayBuffer(12);
var allBytes = new Uint8Array(memory);
var middleBytes = new Uint8Array(memory, 4, 4);
var numberView = new DataView(memory);
var copiedBytes;
var littleEndian = false;

function binaryById(name) { return document.getElementById(name); }
function showBytes() {
  binaryById("byte-output").textContent =
    "Whole buffer:  " + allBytes.join(" ") + "\n" +
    "Middle view:   " + middleBytes.join(" ") + "\n" +
    "Saved slice:   " + copiedBytes.join(" ") + "\n" +
    "View offset: " + middleBytes.byteOffset + " bytes; length: " + middleBytes.byteLength + " bytes\n" +
    "Same buffer: " + (middleBytes.buffer === allBytes.buffer) + "; slice has its own buffer: " + (copiedBytes.buffer !== memory);
}
function showEndian() {
  numberView.setUint16(0, 4660, littleEndian);
  binaryById("endian-output").textContent =
    "Writing: " + (littleEndian ? "little-endian" : "big-endian") + "\n" +
    "First two bytes: " + allBytes.subarray(0, 2).join(" ") + "\n" +
    "Read as big-endian: " + numberView.getUint16(0) + "\n" +
    "Read as little-endian: " + numberView.getUint16(0, true);
  showBytes();
}
function resetBytes() {
  for (var index = 0; index < allBytes.length; index++) allBytes[index] = index;
  copiedBytes = middleBytes.slice();
  showEndian();
}
function encodeText() {
  var input = binaryById("text-input").value;
  var encoded = new TextEncoder().encode(input);
  var split = Math.max(0, encoded.length - 1);
  var decoder = new TextDecoder();
  var output = decoder.decode(encoded.subarray(0, split), {stream: true});
  output += decoder.decode(encoded.subarray(split));
  binaryById("text-output").textContent =
    "UTF-16 code units: " + input.length + "; UTF-8 bytes: " + encoded.length + "\n" +
    "Bytes: " + encoded.join(" ") + "\n" +
    "Decoded: " + output + "\n" +
    "Matches the input: " + (input === output);
}
function countNames() {
  var names = binaryById("collection-input").value.split(",").map(function (name) { return name.trim(); }).filter(function (name) { return name !== ""; });
  var counts = new Map();
  var unique = new Set(names);
  names.forEach(function (name) { counts.set(name, (counts.get(name) || 0) + 1); });
  var lines = [];
  for (var pair of counts) lines.push(pair[0] + ": " + pair[1]);
  binaryById("collection-output").textContent =
    names.length + " names; " + unique.size + " unique\n" + lines.join("\n");
}
function runBinaryChecks() {
  var checks = [
    ["Shared view changes the original", function () { var a = new Uint8Array([1, 2, 3]); var b = a.subarray(1); b[0] = 9; return a[1] === 9; }],
    ["Slice keeps an independent copy", function () { var a = new Uint8Array([1, 2]); var b = a.slice(); b[0] = 9; return a[0] === 1 && b[0] === 9; }],
    ["DataView reads both byte orders", function () { var d = new DataView(new ArrayBuffer(2)); d.setUint16(0, 4660); return d.getUint16(0) === 4660 && d.getUint16(0, true) === 13330; }],
    ["UTF-8 preserves Unicode text", function () { var s = "caf\u00e9 \ud83d\ude00"; return new TextDecoder().decode(new TextEncoder().encode(s)) === s; }],
    ["Map and Set keep insertion order", function () { var m = new Map([["a", 1], ["b", 2]]); m.delete("a"); m.set("a", 3); var keys = []; for (var k of m.keys()) keys.push(k); return keys.join() === "b,a" && new Set([1, 1, 2]).size === 2; }],
    ["Array callbacks preserve holes", function () { var a = [1, , 3].map(function (v) { return v * 2; }); return a.length === 3 && !a.hasOwnProperty(1) && a[2] === 6; }],
    ["Frozen properties stay unchanged", function () { var o = Object.freeze({value: 7}); o.value = 9; return o.value === 7 && Object.isFrozen(o); }],
    ["Sort callbacks compare numeric values", function () { return new Uint16Array([3, 1, 2]).sort(function (a, b) { return a - b; }).join() === "1,2,3"; }]
  ];
  var passed = 0;
  var lines = [];
  checks.forEach(function (check) {
    var ok = check[1]();
    if (ok) passed++;
    lines.push((ok ? "PASS  " : "FAIL  ") + check[0]);
  });
  binaryById("binary-check-output").textContent = lines.join("\n");
  binaryById("binary-status").textContent = passed + " / " + checks.length + " checks passed. Try changing the bytes, text and names below.";
  console.log("Binary workshop: " + passed + " / " + checks.length + " checks passed.");
}

binaryById("write-byte").addEventListener("click", function () {
  var value = Number(binaryById("byte-value").value);
  if (!Number.isInteger(value) || value < 0 || value > 255) {
    binaryById("byte-output").textContent = "Enter a whole byte value from 0 through 255.";
    return;
  }
  middleBytes[0] = value;
  showBytes();
});
binaryById("reset-bytes").addEventListener("click", resetBytes);
binaryById("toggle-endian").addEventListener("click", function () { littleEndian = !littleEndian; showEndian(); });
binaryById("encode-text").addEventListener("click", encodeText);
binaryById("count-names").addEventListener("click", countNames);
binaryById("run-binary-checks").addEventListener("click", runBinaryChecks);
resetBytes();
encodeText();
countNames();
runBinaryChecks();

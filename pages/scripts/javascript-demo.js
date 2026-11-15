// Original examples for ChezCat's first JavaScript subset.
var count = demoSeed;
var colorful = false;
var visible = true;

function byId(id) {
  return document.getElementById(id);
}
function showCount() {
  byId("count").textContent = String(count);
}
function greet() {
  var name = byId("name").value.trim();
  if (name === "") name = "visitor";
  byId("greeting").textContent = "Hello, " + name + ".";
}

byId("more").addEventListener("click", function () {
  count++;
  showCount();
});
byId("less").addEventListener("click", function () {
  count--;
  showCount();
});
byId("zero").addEventListener("click", function () {
  count = demoSeed;
  showCount();
});
byId("name").addEventListener("input", greet);

byId("calculate").addEventListener("click", function () {
  var limit = Number(byId("limit").value);
  if (isNaN(limit) || limit < 1 || limit > 1000 || limit % 1 !== 0) {
    byId("sum").textContent = "Enter a whole number from 1 to 1000.";
    return;
  }
  var total = 0;
  for (var number = 1; number <= limit; number++) {
    total += number;
  }
  byId("sum").textContent = "1 through " + limit + " adds up to " + total + ".";
});

byId("color").addEventListener("click", function () {
  colorful = !colorful;
  if (colorful) {
    byId("preview").className = "preview mint";
    byId("preview").style.color = "#184d41";
  } else {
    byId("preview").className = "preview";
    byId("preview").style.color = "#233747";
  }
});
byId("visibility").addEventListener("click", function () {
  visible = !visible;
  if (visible) {
    byId("preview").style.display = "block";
    byId("view-state").textContent = "The preview is visible.";
  } else {
    byId("preview").style.display = "none";
    byId("view-state").textContent = "The preview is hidden. Click again to restore it.";
  }
});

byId("locked").addEventListener("click", function (event) {
  byId("check-result").textContent = "The handler saw checked = " + byId("locked").checked + ". It cancels the click; the checkbox returns to unchecked.";
  event.preventDefault();
});
byId("local-form").addEventListener("submit", function (event) {
  event.preventDefault();
  byId("form-result").textContent = "Kept on this page: " + byId("message").value;
});
byId("loop").addEventListener("click", function () {
  byId("loop-result").textContent = "Loop entered. When control returns, inspect Ctrl+I and try the counter again.";
  while (true) {}
});

document.addEventListener("DOMContentLoaded", function () {
  showCount();
  greet();
  byId("ready").textContent = "Ready: inline script, external script, then DOMContentLoaded. Starting value: " + demoSeed + ".";
});


// Ordinary page-owned objects and arrays: no special browser demo hooks.
var pieces = {
  title: "Browser pieces",
  items: ["HTML", "CSS"],
  summary: function () {
    return this.title + " (" + this.items.length + "): " + this.items.join(" | ");
  }
};
var sharedPieces = pieces.items;
function addPiece(value) { this.items.push(value); }
function resetPieces(first, second) {
  this.items.length = 0;
  this.items.push(first, second);
}
function showPieces() {
  byId("pieces").textContent = pieces.summary();
  byId("piece-keys").textContent = "Object keys: " + Object.keys(pieces).join(", ") + ". Shared array length: " + sharedPieces.length + ".";
}
byId("add-piece").addEventListener("click", function () {
  var value = byId("piece").value;
  if (value === "" || sharedPieces.length >= 20) { return; }
  addPiece.call(pieces, value);
  showPieces();
});
byId("undo-piece").addEventListener("click", function () {
  sharedPieces.pop();
  showPieces();
});
byId("reset-pieces").addEventListener("click", function () {
  resetPieces.apply(pieces, ["HTML", "CSS"]);
  showPieces();
});
showPieces();

var pendingTimer = null;
byId("timer-start").onclick = () => {
  clearTimeout(pendingTimer);
  byId("timer-result").textContent = "Waiting for the callback...";
  pendingTimer = setTimeout((message) => {
    pendingTimer = null;
    byId("timer-result").textContent = message;
  }, 500, "The timer callback ran.");
};
byId("timer-cancel").onclick = () => {
  clearTimeout(pendingTimer);
  pendingTimer = null;
  byId("timer-result").textContent = "Timer canceled.";
};

function runCompatibilityPass() {
  var numbers = new Array(1, 2, 3);
  var total = 0;
  for (var number of numbers) total += number;
  var rounds = 0;
  do { rounds++; } while (rounds < 3);
  var mask = 1;
  mask |= 2;
  var word = new RegExp("(?<=Chez)Cat(?=/)").exec("ChezCat/0.28")[0];
  var result = eval("demoSeed + total");
  byId("compat-result").textContent = "Sum: " + total + "; rounds: " + rounds + "; bit mask: " + mask + "; match: " + word + "; eval: " + result + ".";
  byId("navigator-result").textContent = "User agent: " + navigator.userAgent + ". Cookies enabled: " + navigator.cookieEnabled + ". Plugin count: " + navigator.plugins.length + ".";
}
byId("compat-run").onclick = runCompatibilityPass;
runCompatibilityPass();

function runJsonExample() {
  try {
    var input = JSON.parse(byId("json-source").value);
    if (!Array.isArray(input.items)) {
      byId("json-result").textContent = "Use an object with an items array.";
      return;
    }
    var result = 0;
    switch (input.mode) {
      case "sum":
        for (var item of input.items) { result += Number(item); }
        break;
      case "count":
        result = input.items.length;
        break;
      default:
        byId("json-result").textContent = "Choose mode sum or count.";
        return;
    }
    byId("json-result").textContent = JSON.stringify({mode: input.mode, result: result});
  } catch (error) {
    byId("json-result").textContent = "Could not read the JSON: " + error.message;
  }
}
byId("json-run").onclick = runJsonExample;
runJsonExample();

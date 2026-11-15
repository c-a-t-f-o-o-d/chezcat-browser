// Original ChezCat DOM game: no canvas, network, storage or external assets.
var player = document.getElementById('player');
var parcel = document.getElementById('parcel');
var patrol = document.getElementById('patrol');
var statusText = document.getElementById('game-status');
var scoreText = document.getElementById('game-score');
var x = 24, y = 112, score = 0, running = false, paused = false;
var frameId = 0, started = 0;
var parcels = [[300,40],[56,40],[300,174],[56,174],[180,112]];

function positionPieces() {
    player.style.left = x + 'px';
    player.style.top = y + 'px';
    parcel.style.left = parcels[Math.min(score,4)][0] + 'px';
    parcel.style.top = parcels[Math.min(score,4)][1] + 'px';
}

function animate(timestamp) {
    frameId = 0;
    if (!running || paused) return;
    var travel = ((timestamp - started) / 8) % 640;
    var px = 20 + (travel < 320 ? travel : 640 - travel);
    patrol.style.left = Math.round(px) + 'px';
    frameId = requestAnimationFrame(animate);
}

function startGame() {
    if (frameId) cancelAnimationFrame(frameId);
    x = 24; y = 112; score = 0; running = true; paused = false;
    parcel.hidden = false;
    started = performance.now();
    statusText.textContent = 'Collect the gold parcels. Arrows or W A S D move; Space pauses.';
    scoreText.textContent = 'Parcels: 0 / 5';
    positionPieces();
    frameId = requestAnimationFrame(animate);
}

function pauseGame() {
    if (!running) return;
    paused = !paused;
    if (paused) {
        if (frameId) cancelAnimationFrame(frameId);
        frameId = 0;
        statusText.textContent = 'Paused. Press Space or Pause / resume to continue.';
    } else {
        statusText.textContent = 'Collect the gold parcels. Arrows or W A S D move; Space pauses.';
        frameId = requestAnimationFrame(animate);
    }
}

function move(dx,dy) {
    if (!running || paused) return;
    x = Math.max(0,Math.min(350,x+dx));
    y = Math.max(0,Math.min(230,y+dy));
    var target = parcels[score];
    if (x < target[0]+18 && x+28 > target[0] && y < target[1]+18 && y+28 > target[1]) {
        score++;
        scoreText.textContent = 'Parcels: ' + score + ' / 5';
        if (score === 5) {
            running = false;
            if (frameId) cancelAnimationFrame(frameId);
            frameId = 0;
            parcel.hidden = true;
            statusText.textContent = 'All five delivered. Choose Start / restart for another round.';
        }
    }
    positionPieces();
}

document.getElementById('start-game').onclick = startGame;
document.getElementById('pause-game').onclick = pauseGame;
document.getElementById('move-left').onclick = function(){move(-16,0);};
document.getElementById('move-up').onclick = function(){move(0,-16);};
document.getElementById('move-down').onclick = function(){move(0,16);};
document.getElementById('move-right').onclick = function(){move(16,0);};
document.addEventListener('keydown',function(event){
    if (event.ctrlKey || event.altKey || event.metaKey) return;
    var code = event.code;
    if (code === 'ArrowLeft' || code === 'KeyA') { event.preventDefault(); move(-16,0); }
    else if (code === 'ArrowRight' || code === 'KeyD') { event.preventDefault(); move(16,0); }
    else if (code === 'ArrowUp' || code === 'KeyW') { event.preventDefault(); move(0,-16); }
    else if (code === 'ArrowDown' || code === 'KeyS') { event.preventDefault(); move(0,16); }
    else if (code === 'Space') { event.preventDefault(); if (!event.repeat) pauseGame(); }
});

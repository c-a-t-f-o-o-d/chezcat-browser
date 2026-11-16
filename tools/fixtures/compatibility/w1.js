Promise.resolve().then(function () {
  setTimeout(function () {
    var id = document.getElementById('response-id').textContent;
    document.getElementById('async-state').textContent = 'W1 async ready response=' + id + ' history=' + history.length;
  }, 30);
});

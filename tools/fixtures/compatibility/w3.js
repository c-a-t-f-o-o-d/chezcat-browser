new Vue({
  el: '#app',
  data: { count: 0, name: '', phase: 'ready' },
  render: function (h) {
    var self = this;
    return h('div', { attrs: { id: 'mounted-panel' } }, [
      h('p', { attrs: { id: 'reactive-state' } }, ['W3 reactive count=' + this.count + ' name=' + this.name + ' phase=' + this.phase]),
      h('label', ['Name ', h('input', { attrs: { name: 'reactive-name', type: 'text' }, domProps: { value: this.name }, on: { input: function (input) { self.name = input.target.value; } } })]),
      h('button', { attrs: { name: 'increment', type: 'button' }, on: { click: this.increment } }, ['Increment original counter']),
      h('button', { attrs: { name: 'async-update', type: 'button' }, on: { click: this.updateAsync } }, ['Complete asynchronous update'])
    ]);
  },
  methods: {
    increment: function (input) {
      if (!input.isTrusted) throw new Error('Counter requires trusted App input');
      this.count += 1;
    },
    updateAsync: function (input) {
      if (!input.isTrusted) throw new Error('Async update requires trusted App input');
      var self = this;
      this.phase = 'pending';
      Promise.resolve().then(function () {
        setTimeout(function () {
          self.phase = 'complete';
          self.$nextTick(function () { document.title = 'W3 asynchronous Vue update complete'; });
        }, 30);
      });
    }
  },
  mounted: function () { document.getElementById('mount-state').textContent = 'W3 runtime mounted Vue ' + Vue.version; }
});

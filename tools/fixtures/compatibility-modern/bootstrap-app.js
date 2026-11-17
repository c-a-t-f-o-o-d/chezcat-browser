if (typeof bootstrap === 'undefined' || bootstrap.Dropdown.VERSION !== '5.3.9') throw new Error('Expected pinned Bootstrap 5.3.9');
var toggle = document.getElementById('modern-menu-toggle');
toggle.addEventListener('shown.bs.dropdown', function () { document.getElementById('menu-state').textContent = 'Bootstrap dropdown open'; });
toggle.addEventListener('hidden.bs.dropdown', function () { document.getElementById('menu-state').textContent = 'Bootstrap dropdown closed'; });
document.getElementById('modern-menu-choice').addEventListener('click', function (event) {
  if (!event.isTrusted) throw new Error('Expected a genuine trusted original click');
  document.getElementById('input-state').textContent = 'Bootstrap trusted choice amber';
});
var dropdown = new bootstrap.Dropdown(toggle);
document.getElementById('init-state').textContent = 'Bootstrap initialized version=' + bootstrap.Dropdown.VERSION;

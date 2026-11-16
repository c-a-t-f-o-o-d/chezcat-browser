jQuery(function ($) {
  if (typeof $.fn.dropdown !== 'function') throw new Error('Bootstrap dropdown did not initialize');
  $('#init-state').text('W2 initialized jquery=' + $.fn.jquery + ' bootstrap=' + $.fn.dropdown.Constructor.VERSION);
  $('#library-menu').on('shown.bs.dropdown', function () { $('#menu-state').text('W2 dropdown open'); });
  $('#library-menu').on('hidden.bs.dropdown', function () { $('#menu-state').text('W2 dropdown closed'); });
  $('#choose-amber').on('click', function (event) {
    if (!event.originalEvent || !event.originalEvent.isTrusted) throw new Error('Expected a genuine trusted original click');
    $('#query').val('amber 雪');
    $('#selected-state').text('W2 trusted choice amber 雪');
    setTimeout(function () { $('#async-state').text('W2 asynchronous library text update complete'); }, 30);
  });
});

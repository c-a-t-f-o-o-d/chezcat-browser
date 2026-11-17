jQuery(function ($) {
  if ($.fn.jquery !== '4.0.0') throw new Error('Expected pinned jQuery 4.0.0');
  $('#init-state').text('jQuery initialized version=' + $.fn.jquery);
  $('#jquery-apply').on('click', function (event) {
    if (!event.originalEvent || !event.originalEvent.isTrusted) throw new Error('Expected a genuine trusted original click');
    $('#input-state').text('jQuery trusted update ' + $('#modern-name').val());
    setTimeout(function () { $('#async-state').text('jQuery asynchronous update complete'); }, 30);
  });
});

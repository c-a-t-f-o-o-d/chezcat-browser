"""Author original visible VP8 I/P video and finite Matroska/PCM16 fixtures.

Original deterministic RGB and PCM are created here. Installed FFmpeg/libvpx is
development-only encoding; FFmpeg's native VP8 decoder independently produces
the full decoded references. No product executable is run and no external codec
implementation is copied into project source.
The resulting fixture bytes never require FFmpeg or an OS codec at runtime.
"""
import argparse
import hashlib
import json
import math
from pathlib import Path
import shutil
import struct
import subprocess
import sys
import tempfile


WIDTH, HEIGHT, RATE, FRAMES = 64, 48, 8, 24
SAMPLE_RATE, CHANNELS, AUDIO_PACKET_FRAMES = 48000, 2, 6000


def sha(data):
    return hashlib.sha256(data).hexdigest()


def scene():
    pixels = bytearray()
    for frame in range(FRAMES):
        for y in range(HEIGHT):
            for x in range(WIDTH):
                pan = (x + frame * 2) % 96
                stripe = ((pan // 8) + (y // 8)) % 2
                color = (32 + pan, 48 + y * 2, 64 + stripe * 80)
                if 4 + frame % 9 <= x < 20 + frame % 9 and 16 <= y < 32:
                    color = (224, 48 + frame * 3, 32)
                if y >= 40:
                    color = (24, 176, 32 + frame * 5)
                pixels.extend(color)
    return bytes(pixels)


def pcm():
    count = FRAMES * SAMPLE_RATE // RATE
    samples = bytearray()
    for frame in range(count):
        envelope = min(1, frame / 2400, (count - 1 - frame) / 2400)
        left = round(2400 * envelope * math.sin(2 * math.pi * 220 * frame / SAMPLE_RATE))
        right = round(2000 * envelope * math.sin(2 * math.pi * 330 * frame / SAMPLE_RATE))
        samples.extend(struct.pack('<hh', left, right))
    return bytes(samples)


def run(command, *, payload=None, timeout=30, check=True):
    options = {'creationflags': subprocess.CREATE_NO_WINDOW} if sys.platform == 'win32' else {}
    result = subprocess.run(command, input=payload, stdout=subprocess.PIPE,
        stderr=subprocess.PIPE, timeout=timeout, check=False, **options)
    if check and result.returncode:
        raise RuntimeError(result.stderr.decode('utf-8', errors='replace'))
    return result


def packets(data):
    if data[:4] != b'DKIF' or data[8:12] != b'VP80':
        raise ValueError('Expected genuine IVF/VP8 output.')
    result, offset = [], 32
    while offset < len(data):
        size, tick = struct.unpack_from('<IQ', data, offset)
        offset += 12
        payload = data[offset:offset + size]
        offset += size
        if len(payload) != size or len(payload) < 3:
            raise ValueError('Truncated encoder output.')
        tag = int.from_bytes(payload[:3], 'little')
        result.append({'timestamp': tick, 'payload': payload, 'keyframe': not bool(tag & 1),
            'version': (tag >> 1) & 7, 'show': bool(tag & 16),
            'first_partition_bytes': tag >> 5})
    return result


def ivf_subset(header, records):
    data = bytearray(header)
    struct.pack_into('<I', data, 24, len(records))
    origin = records[0]['timestamp']
    for row in records:
        payload = row['payload']
        data.extend(struct.pack('<IQ', len(payload), row['timestamp'] - origin))
        data.extend(payload)
    return bytes(data)


def size(value):
    for width in range(1, 9):
        if value < (1 << (7 * width)) - 1:
            return ((1 << (7 * width)) | value).to_bytes(width, 'big')
    raise ValueError('Original finite element is too large.')


def element(identifier, body):
    return identifier.to_bytes((identifier.bit_length() + 7) // 8, 'big') + size(len(body)) + body


def uint(identifier, value):
    return element(identifier, value.to_bytes(max(1, (value.bit_length() + 7) // 8), 'big'))


def matroska(records, samples):
    # Project-original stdlib EBML authoring, retaining the existing finite
    # Matroska/PCM profile. Only the video SimpleBlock key flag varies for P data.
    duration_ms = FRAMES * 1000 // RATE
    header = element(0x1a45dfa3, uint(0x4286, 1) + uint(0x42f7, 1)
        + uint(0x42f2, 4) + uint(0x42f3, 8) + element(0x4282, b'matroska')
        + uint(0x4287, 4) + uint(0x4285, 2))
    info = element(0x1549a966, uint(0x2ad7b1, 1000000)
        + element(0x4489, struct.pack('>d', duration_ms))
        + element(0x4d80, b'ChezCat original fixture')
        + element(0x5741, b'ChezCat original fixture'))
    video = uint(0xd7, 1) + uint(0x73c5, 1) + uint(0x83, 1) + uint(0x9c, 0)
    video += element(0x86, b'V_VP8') + uint(0x23e383, 1000000000 // RATE)
    video += element(0xe0, uint(0xb0, WIDTH) + uint(0xba, HEIGHT))
    audio = uint(0xd7, 2) + uint(0x73c5, 2) + uint(0x83, 2) + uint(0x9c, 0)
    audio += element(0x86, b'A_PCM/INT/LIT')
    audio += element(0xe1, element(0xb5, struct.pack('>d', SAMPLE_RATE))
        + uint(0x9f, CHANNELS) + uint(0x6264, 16))
    tracks = element(0x1654ae6b, element(0xae, video) + element(0xae, audio))
    clusters = bytearray()
    for index, row in enumerate(records):
        flags = 0x80 if row['keyframe'] else 0
        blocks = element(0xa3, b'\x81' + struct.pack('>hB', 0, flags) + row['payload'])
        begin = index * AUDIO_PACKET_FRAMES * CHANNELS * 2
        block = samples[begin:begin + AUDIO_PACKET_FRAMES * CHANNELS * 2]
        if len(block) != AUDIO_PACKET_FRAMES * CHANNELS * 2:
            raise ValueError('Original PCM block must fill one video interval.')
        blocks += element(0xa3, b'\x82' + struct.pack('>hB', 0, 0x80) + block)
        clusters.extend(element(0x1f43b675, uint(0xe7, index * 1000 // RATE) + blocks))
    return header + element(0x18538067, info + tracks + clusters)


def decoded(ffmpeg, source, format_name='rgba', check=True):
    return run([ffmpeg, '-hide_banner', '-loglevel', 'error', '-c:v', 'vp8',
        '-i', str(source), '-map', '0:v:0', '-fps_mode', 'passthrough',
        '-f', 'rawvideo', '-pix_fmt', format_name, 'pipe:1'], check=check)


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--ffmpeg', default=shutil.which('ffmpeg'))
    parser.add_argument('--ffprobe', default=shutil.which('ffprobe'))
    parser.add_argument('--output-dir', type=Path, default=Path(__file__).resolve().parent / 'fixtures')
    args = parser.parse_args()
    if not args.ffmpeg or not args.ffprobe:
        parser.error('Choose existing development FFmpeg and ffprobe tools.')
    out = args.output_dir.resolve()
    out.mkdir(parents=True, exist_ok=True)
    raw, samples = scene(), pcm()
    video = out / 'scene-inter.ivf'
    command = [args.ffmpeg, '-hide_banner', '-loglevel', 'info', '-f', 'rawvideo',
        '-pixel_format', 'rgb24', '-video_size', f'{WIDTH}x{HEIGHT}', '-framerate', str(RATE),
        '-i', 'pipe:0', '-an', '-c:v', 'libvpx', '-profile:v', '0', '-pix_fmt', 'yuv420p',
        '-deadline', 'realtime', '-cpu-used', '16', '-auto-alt-ref', '0', '-lag-in-frames', '0',
        '-g', '12', '-keyint_min', '12', '-slices', '1', '-crf', '10', '-b:v', '200k',
        '-threads', '1', '-f', 'ivf', '-y', str(video)]
    encoded = run(command, payload=raw)
    data = video.read_bytes()
    if (struct.unpack_from('<HH', data, 4) != (0, 32)
        or struct.unpack_from('<HHII', data, 12) != (WIDTH, HEIGHT, RATE, 1)
        or struct.unpack_from('<I', data, 24)[0] != FRAMES):
        raise ValueError('Actual IVF header differs from the selected dimensions/timebase/count.')
    rows = packets(data)
    keys = [index for index, row in enumerate(rows) if row['keyframe']]
    if len(rows) != FRAMES or keys != [0, 12] or any(
        row['version'] != 0 or not row['show'] or row['timestamp'] != index
        for index, row in enumerate(rows)):
        raise ValueError('Actual encoded tags differ from the declared visible version-0 two-GOP profile; retain evidence and inspect.')
    if any(row['payload'][3:6] != b'\x9d\x01\x2a'
        or struct.unpack_from('<HH', row['payload'], 6) != (WIDTH, HEIGHT)
        for row in rows if row['keyframe']):
        raise ValueError('Actual keyframe dimensions/scaling differ from the selected profile.')
    reference = decoded(args.ffmpeg, video).stdout
    yuv = decoded(args.ffmpeg, video, 'yuv420p').stdout
    stride = WIDTH * HEIGHT * 4
    if len(reference) != FRAMES * stride or len(yuv) != FRAMES * WIDTH * HEIGHT * 3 // 2:
        raise ValueError('Independent decoder did not emit all complete reference frames.')
    (out / 'scene-inter.rgba').write_bytes(reference)
    (out / 'scene-inter.yuv').write_bytes(yuv)
    display_command = [args.ffmpeg] + ['-hide_banner', '-loglevel', 'error', '-f', 'rawvideo', '-pixel_format', 'yuv420p', '-video_size', '64x48', '-framerate', '8', '-i', 'scene-inter.yuv', '-vf', 'scale=64:48:flags=bilinear+accurate_rnd+full_chroma_int+full_chroma_inp:in_color_matrix=bt601:in_range=tv:out_range=pc:in_h_chr_pos=128:in_v_chr_pos=128', '-fps_mode', 'passthrough', '-f', 'rawvideo', '-pix_fmt', 'rgba', 'pipe:1']
    display_command[display_command.index('-i') + 1] = str(out / 'scene-inter.yuv')
    centered = run(display_command).stdout
    if len(centered) != len(reference):
        raise ValueError('Independent centered display reference has the wrong extent.')
    (out / 'scene-inter-centered.rgba').write_bytes(centered)
    display_metadata = {'schema_version': 1, 'width': 64, 'height': 48, 'frames': 24, 'rate': 8, 'scale': 1, 'input_yuv': 'scene-inter.yuv', 'input_yuv_sha256': '674b0476f912fe06d3237af680c78057fed8ae5a201cb1496c7df06875f889e0', 'retained_default_rgba': 'scene-inter.rgba', 'retained_default_rgba_sha256': '186f6fd69c95beb5ffa1e99c29a44615affcec075ff869e5155963e541017420', 'reference_rgba': 'scene-inter-centered.rgba', 'reference_rgba_sha256': 'b38ff672cbe99d281ea8ad278f505c1a72332a43af1b0dba84aefdf0192316ae', 'independent_command': ['ffmpeg', '-hide_banner', '-loglevel', 'error', '-f', 'rawvideo', '-pixel_format', 'yuv420p', '-video_size', '64x48', '-framerate', '8', '-i', 'scene-inter.yuv', '-vf', 'scale=64:48:flags=bilinear+accurate_rnd+full_chroma_int+full_chroma_inp:in_color_matrix=bt601:in_range=tv:out_range=pc:in_h_chr_pos=128:in_v_chr_pos=128', '-fps_mode', 'passthrough', '-f', 'rawvideo', '-pix_fmt', 'rgba', 'pipe:1'], 'ffmpeg_version': 'N-124319-gb801f1fe6d-20260503', 'tool_use': 'Development-only; no external codec implementation or executable is included in product.', 'primary_reference': 'https://www.rfc-editor.org/rfc/rfc6386.html#section-5', 'render_profile': {'matrix': 'BT.601', 'input_range': 'limited', 'output_range': 'full', 'chroma_upsampling': 'centered bilinear', 'horizontal_chroma_position': 128, 'vertical_chroma_position': 128}, 'pixel_tolerance': {'mean_rgb_error_less_than': 0.5, 'maximum_channel_error_at_most': 2}, 'scope': 'Separate independent centered-bilinear BT.601 presentation reference; original default RGBA and normative YUV references remain distinct.'}
    display_metadata.update(input_yuv_sha256=sha(yuv), retained_default_rgba_sha256=sha(reference),
        reference_rgba_sha256=sha(centered), ffmpeg_version=run([args.ffmpeg, '-version']).stdout.decode('utf-8').splitlines()[0].split(' version ', 1)[-1].split(' Copyright', 1)[0])
    (out / 'scene-inter-display.json').write_text(json.dumps(display_metadata, indent=2) + '\n', encoding='utf-8', newline='\n')
    muxed = matroska(rows, samples)
    mux = out / 'scene-inter-av.mkv'
    mux.write_bytes(muxed)
    (out / 'scene-inter-av.pcm').write_bytes(samples)
    av_pixels = decoded(args.ffmpeg, mux).stdout
    av_samples = run([args.ffmpeg, '-hide_banner', '-loglevel', 'error', '-i', str(mux),
        '-map', '0:a:0', '-c:a', 'pcm_s16le', '-f', 's16le', 'pipe:1']).stdout
    if av_pixels != reference or av_samples != samples:
        raise ValueError('Independent Matroska decode differs from IVF/author PCM references.')
    with tempfile.TemporaryDirectory(prefix='chezcat-fixture-') as temporary:
        probes = Path(temporary)
        no_key = probes / 'without-initial-key.ivf'
        no_key.write_bytes(ivf_subset(data[:32], rows[1:12]))
        no_key_result = decoded(args.ffmpeg, no_key, check=False)
        reset = probes / 'second-key-reset.ivf'
        reset.write_bytes(ivf_subset(data[:32], rows[12:]))
        reset_pixels = decoded(args.ffmpeg, reset).stdout
        dropped = probes / 'without-first-p.ivf'
        dropped.write_bytes(ivf_subset(data[:32], rows[:1] + rows[2:12]))
        dropped_pixels = decoded(args.ffmpeg, dropped).stdout
        if len(dropped_pixels) != 11 * stride:
            raise ValueError('Dependency probe did not return the remaining eleven frames.')
        changed = [index for index in range(2, 12)
            if dropped_pixels[(index - 1) * stride:index * stride] != reference[index * stride:(index + 1) * stride]]
        checks = {'all_frames_independently_decoded': len(reference) == FRAMES * stride,
            'all_reference_frames_visibly_change': all(reference[i * stride:(i + 1) * stride]
                != reference[(i - 1) * stride:i * stride] for i in range(1, FRAMES)),
            'interframes_need_a_reference_key': len(no_key_result.stdout) == 0,
            'dropping_first_p_changes_later_pixels': bool(changed),
            'second_key_resets_full_decoder_state': reset_pixels == reference[12 * stride:],
            'mux_preserves_all_video_pixels': av_pixels == reference,
            'mux_pcm_matches_independent_author_reference': av_samples == samples}
    if not all(checks.values()):
        raise ValueError('Original fixture dependency/reset/oracle validation failed; inspect process output.')
    frame_records = []
    for index, row in enumerate(rows):
        pixels = reference[index * stride:(index + 1) * stride]
        frame_records.append({key: value for key, value in row.items() if key != 'payload'}
            | {'bytes': len(row['payload']), 'payload_sha256': sha(row['payload']),
                'reference_rgba_sha256': sha(pixels)})
    metadata = {'scope': __doc__, 'width': WIDTH, 'height': HEIGHT, 'rate': RATE, 'scale': 1,
        'duration_ms': FRAMES * 1000 // RATE, 'frames': frame_records, 'keyframe_indices': keys,
        'source_rgb_sha256': sha(raw), 'ivf_sha256': sha(data), 'reference_rgba_sha256': sha(reference),
        'reference_yuv420p_sha256': sha(yuv),
        'encoder_command': ['ffmpeg'] + command[1:-1] + ['scene-inter.ivf'],
        'independent_decoder': 'FFmpeg native vp8, distinct from the libvpx encoder',
        'ffmpeg_version': run([args.ffmpeg, '-version']).stdout.decode('utf-8').splitlines()[0],
        'development_tool_license': 'Installed FFmpeg reports GPL version 3 or later; development tool not redistributed or used by production.',
        'production_dependencies_added': []}
    (out / 'scene-inter.json').write_text(json.dumps(metadata, indent=2) + '\n', encoding='utf-8', newline='\n')
    av_metadata = {'scope': 'Project-original finite Matroska V_VP8 and A_PCM/INT/LIT with original stereo PCM16.',
        'mkv_sha256': sha(muxed), 'pcm_sha256': sha(samples), 'duration_ms': FRAMES * 1000 // RATE,
        'video_packet_sha256': [sha(row['payload']) for row in rows], 'video_frames': FRAMES,
        'pcm_frames': len(samples) // (CHANNELS * 2), 'sample_rate': SAMPLE_RATE,
        'channels': CHANNELS, 'bits': 16, 'audio_packet_frames': AUDIO_PACKET_FRAMES,
        'video_reference': 'scene-inter.rgba', 'video_reference_sha256': sha(reference),
        'pcm_reference': 'scene-inter-av.pcm'}
    (out / 'scene-inter-av.json').write_text(json.dumps(av_metadata, indent=2) + '\n', encoding='utf-8', newline='\n')
    probe = run([args.ffprobe, '-v', 'error', '-show_streams', '-show_frames', '-of', 'json', str(video)])
    if len(json.loads(probe.stdout).get('frames', [])) != FRAMES:
        raise ValueError('Independent frame inspection did not report the declared frame count.')
    print(json.dumps({'checks': checks, 'frames': len(rows), 'keyframes': keys,
        'ivf_sha256': sha(data), 'mkv_sha256': sha(muxed)}, indent=2))
    return 0 if all(checks.values()) else 1


if __name__ == '__main__':
    raise SystemExit(main())

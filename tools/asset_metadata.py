"""Bounded metadata policy for published branding images.

Only rendering fields of the reviewed PNG and ICO formats are accepted. This
does not inspect pixel content or change an image; original codec fixtures are
outside this branding policy.
"""
from pathlib import PurePosixPath
import struct
import zlib

MAX_ASSET_BYTES=16*1024*1024
MAX_PNG_CHUNKS=8192
MAX_ICON_IMAGES=256
PNG_SIGNATURE=b'\x89PNG\r\n\x1a\n'
PNG_RENDER_CHUNKS={b'IHDR',b'PLTE',b'IDAT',b'IEND',b'cHRM',b'gAMA',b'sBIT',
                   b'sRGB',b'bKGD',b'hIST',b'tRNS',b'pHYs'}


def check_png(data):
    if len(data)>MAX_ASSET_BYTES or not data.startswith(PNG_SIGNATURE):
        raise ValueError('Invalid or oversized branding PNG.')
    at=len(PNG_SIGNATURE);chunks=0;image_data=False;dimensions=None
    palette=0;seen=set();depth=None;color=None
    while at<len(data):
        chunks+=1
        if chunks>MAX_PNG_CHUNKS or len(data)-at<12:
            raise ValueError('Truncated or excessive branding PNG chunks.')
        size=int.from_bytes(data[at:at+4],'big');kind=data[at+4:at+8]
        end=at+12+size
        if end>len(data):raise ValueError('Truncated branding PNG chunk.')
        body=data[at+8:end-4]
        if zlib.crc32(kind+body)&0xffffffff!=int.from_bytes(data[end-4:end],'big'):
            raise ValueError('Invalid branding PNG chunk checksum.')
        if kind not in PNG_RENDER_CHUNKS:
            raise ValueError('Branding PNG contains unapproved non-rendering metadata.')
        if chunks==1:
            if kind!=b'IHDR' or size!=13:
                raise ValueError('Branding PNG needs its first image header.')
            width,height=struct.unpack('>II',body[:8])
            if not width or not height:raise ValueError('Empty branding PNG dimensions.')
            depth,color,compression,filtering,interlace=body[8:]
            depths={0:(1,2,4,8,16),2:(8,16),3:(1,2,4,8),4:(8,16),6:(8,16)}
            if depth not in depths.get(color,()) or compression or filtering or interlace not in (0,1):
                raise ValueError('Invalid branding PNG rendering header.')
            dimensions=(width,height)
        elif kind==b'IHDR':raise ValueError('Duplicate branding PNG header.')
        if kind!=b'IDAT' and kind in seen:
            raise ValueError('Duplicate branding PNG rendering field.')
        if image_data and kind not in (b'IDAT',b'IEND'):
            raise ValueError('Branding PNG rendering field follows image data.')
        if kind==b'PLTE':
            if color in (0,4) or not size or size%3 or size>768 or color==3 and size//3>1<<depth:
                raise ValueError('Invalid branding PNG palette.')
            palette=size//3
        elif kind in (b'cHRM',b'gAMA',b'sRGB',b'sBIT'):
            if palette:raise ValueError('Branding PNG color description follows its palette.')
            if kind==b'cHRM':
                if size!=32:raise ValueError('Invalid branding PNG chromaticity fields.')
                values=struct.unpack('>8I',body)
                if any(x+y>100000 for x,y in zip(values[::2],values[1::2])):
                    raise ValueError('Invalid branding PNG chromaticity values.')
            elif kind==b'gAMA':
                if size!=4 or not int.from_bytes(body,'big'):
                    raise ValueError('Invalid branding PNG gamma field.')
            elif kind==b'sRGB':
                if size!=1 or body[0]>3:raise ValueError('Invalid branding PNG rendering intent.')
            else:
                channels={0:1,2:3,3:3,4:2,6:4}[color]
                if size!=channels or any(not value or value>(8 if color==3 else depth) for value in body):
                    raise ValueError('Invalid branding PNG significant-bit fields.')
        elif kind==b'pHYs':
            if size!=9 or body[8]>1:raise ValueError('Invalid branding PNG pixel-density fields.')
        elif kind==b'hIST':
            if not palette or size!=2*palette:raise ValueError('Invalid branding PNG histogram fields.')
        elif kind in (b'bKGD',b'tRNS'):
            if color==3:
                valid=palette and (size==1 and body[0]<palette if kind==b'bKGD' else 0<size<=palette)
            else:
                channels=1 if color in (0,4) else 3
                valid=(kind!=b'tRNS' or color in (0,2)) and size==2*channels
                if valid:valid=all(int.from_bytes(body[at:at+2],'big')<1<<depth for at in range(0,size,2))
            if not valid:raise ValueError('Invalid branding PNG background or transparency fields.')
        if kind==b'IDAT':
            if color==3 and not palette:raise ValueError('Indexed branding PNG has no palette.')
            image_data=True
        if kind==b'IEND':
            if size or not image_data or end!=len(data):
                raise ValueError('Branding PNG has incomplete or appended data.')
            return dimensions
        seen.add(kind)
        at=end
    raise ValueError('Branding PNG has no complete end chunk.')


def check_icon_bitmap(data,width,height):
    # The reviewed uncompressed DIB layout has no optional metadata blocks.
    if len(data)<40:raise ValueError('Truncated branding icon bitmap.')
    size,w,h,planes,bits,compression,image_bytes,_,_,colors,_=struct.unpack('<IiiHHIIiiII',data[:40])
    if size!=40 or w!=width or h!=2*height or planes!=1 or bits not in (1,4,8,16,24,32) or compression:
        raise ValueError('Unapproved branding icon bitmap layout.')
    if bits>8 and colors or bits<=8 and colors>(1<<bits):
        raise ValueError('Invalid branding icon palette.')
    palette=(colors or (1<<bits)) if bits<=8 else 0
    pixels=((width*bits+31)//32)*4*height
    mask=((width+31)//32)*4*height
    if image_bytes not in (0,pixels,pixels+mask) or len(data)!=40+4*palette+pixels+mask:
        raise ValueError('Branding icon bitmap has incomplete or appended data.')
    return True


def check_icon(data):
    if len(data)>MAX_ASSET_BYTES or len(data)<6:
        raise ValueError('Invalid or oversized branding icon.')
    reserved,kind,count=struct.unpack('<HHH',data[:6])
    if reserved or kind!=1 or not 1<=count<=MAX_ICON_IMAGES:
        raise ValueError('Unapproved branding icon header.')
    end=6+16*count
    if end>len(data):raise ValueError('Truncated branding icon directory.')
    images=[]
    for index in range(count):
        at=6+16*index
        width,height,_,zero,_,_,size,offset=struct.unpack('<BBBBHHII',data[at:at+16])
        if zero or not size or offset<end or offset+size>len(data):
            raise ValueError('Invalid branding icon image extent.')
        images.append((offset,size,width or 256,height or 256))
    for offset,size,width,height in sorted(images):
        if offset!=end:raise ValueError('Branding icon has gaps or overlapping images.')
        body=data[offset:offset+size]
        if body.startswith(PNG_SIGNATURE):
            if check_png(body)!=(width,height):
                raise ValueError('Branding icon and PNG dimensions disagree.')
        else:check_icon_bitmap(body,width,height)
        end=offset+size
    if end!=len(data):raise ValueError('Branding icon has appended data.')
    return True


def check_payload(name,data):
    if len(data)>MAX_ASSET_BYTES:raise ValueError('Branding asset exceeds its finite inspection allowance.')
    suffix=PurePosixPath(name).suffix.casefold()
    if suffix=='.png':check_png(data)
    elif suffix=='.ico':check_icon(data)
    elif suffix=='.md':
        try:text=data.decode('utf-8')
        except UnicodeError:raise ValueError('Branding documentation must be UTF-8 text.') from None
        if '\0' in text:raise ValueError('Branding documentation contains binary data.')
    else:raise ValueError('Unreviewed branding asset format.')
    return True

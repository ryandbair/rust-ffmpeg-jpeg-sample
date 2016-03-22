extern crate ffmpeg;
extern crate image;

use std::env;
use ffmpeg::{codec, format, media, util, Packet};

use std::io::Write;

fn main() {
    ffmpeg::init().unwrap();
    format::network::init();

    let input = env::args().nth(1).expect("specify input");
    let jpeg_out = env::args().nth(2).expect("specify output file");

    // Open the input, ffmpeg handles all the format selection, etc for us
    let mut input_ctx = format::input(&input).unwrap();

    // we need to scope this since input_ctx.streams() borrows the reference immutably
    // but we need it mutably below when we retrieve the packets
    let (idx, mut codec) = {
        let input_stream = input_ctx.streams().best(media::Type::Video).expect("failed to find video stream");
        let input_codec = input_stream.codec().decoder().video().unwrap();

        (input_stream.index(), input_codec)
    };

    // TODO: try filter_map here, it might look a little cleaner
    let video_packets =
        input_ctx.packets()
                 .skip_while(|&(_, ref packet)| !packet.is_key())
                 .filter(|&(ref stream, _)| stream.index() == idx)
                 .map(|(_, packet)| packet)
                 .take(500);

    let enc_codec = codec::encoder::find(codec::Id::H264).unwrap();

    let video_file = format!("{}/out.m4v", jpeg_out);
    /* The wrapper only exposes the format as a stringly typed parameter. Given the underlying API,
     * I'm not sure how much better this can get.
     */
    let mut output_ctx = format::output_as(&std::path::Path::new(&video_file), "m4v").unwrap();

    let (mut encoder, out_idx) = {
        let mut output_stream = output_ctx.add_stream(enc_codec).unwrap();

        /* output_stream.codec() is the codec context, same as ost->codec in the C API
         * The encoder().video() calls assert the codec type and constraint the operations to ones
         * that make sense in that context.
         */
        let mut video_out = output_stream.codec().encoder().video().unwrap();

        video_out.set_width(codec.width());
        video_out.set_height(codec.height());
        video_out.set_format(format::Pixel::YUV422P);
        video_out.set_time_base(codec.time_base());
        video_out.set_bit_rate(64000);

        let mut encoder = video_out.open_as(enc_codec).unwrap();
        encoder.set_time_base(codec.time_base());
        output_stream.set_time_base(codec.time_base());

        (encoder, output_stream.index())
    };

    output_ctx.write_header().unwrap();

    let mut frame = util::frame::Video::new(codec.format(), codec.width(), codec.height());
    let mut rgb_frame = util::frame::Video::new(util::format::Pixel::RGB24, codec.width(), codec.height());
    let mut yuv_frame = util::frame::Video::new(util::format::Pixel::YUV422P, codec.width(), codec.height());

    let mut pts = 0;

    for packet in video_packets {
        match codec.decode(&packet, &mut frame) {
            Err(e) => println!("error decoding packet {:?}", e),
            Ok(false) => (),// OK, couldn't decode
            Ok(true) => {// we got output
                let mut encoded_packet = Packet::empty();

                let frame_time = frame.pts().unwrap_or_else(|| frame.timestamp().unwrap());
                writeln!(&mut std::io::stderr(), "got a frame, size is: {}x{}", frame.width(), frame.height());

                let mut yuv_converter = frame.converter(format::Pixel::YUV422P).unwrap();
                yuv_converter.run(&frame, &mut yuv_frame).unwrap();

                yuv_frame.set_pts(Some(pts));
                pts += 1;

                match encoder.encode(&yuv_frame, &mut encoded_packet) {
                    Ok(true) => {
                        // We must set the stream per ffmpeg docs
                        encoded_packet.set_stream(out_idx);
                        encoded_packet.write(&mut output_ctx).unwrap();
                    },
                    Ok(false) => println!("false when encoding"),
                    Err(e) => println!("error encoding: {:?}", e),
                }

                if packet.is_key() {
                    let out_name = format!("{}/{}.jpg", jpeg_out, frame_time);

                    // creates a sws context converts from the current pixel
                    // format to the specified, without resize
                    let mut rgb_converter = frame.converter(format::Pixel::RGB24).unwrap();
                    rgb_converter.run(&frame, &mut rgb_frame).unwrap();

                    let rgb_buffer = rgb_frame.data(0);
                    image::save_buffer(out_name, rgb_buffer, rgb_frame.width(), rgb_frame.height(), image::RGB(8)).unwrap();
                }
            }
        };
    }

    encoder.flush(&mut Packet::empty()).unwrap();
    output_ctx.write_trailer().unwrap();
}

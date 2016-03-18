extern crate ffmpeg;
extern crate image;

use std::env;
use ffmpeg::{format, media, util};

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
                 .filter(|&(ref stream, ref packet)| stream.index() == idx && packet.is_key())
                 .map(|(_, packet)| packet);

    let mut frame = util::frame::Video::new(codec.format(), codec.width(), codec.height());
    let mut rgb_frame = util::frame::Video::new(util::format::Pixel::RGB24, codec.width(), codec.height());

    for packet in video_packets {
        match codec.decode(&packet, &mut frame) {
            Err(e) => println!("error decoding packet {:?}", e),
            Ok(false) => (),// OK, couldn't decode
            Ok(true) => {// we got output
                let frame_time = frame.pts().unwrap_or_else(|| frame.timestamp().unwrap());
                writeln!(&mut std::io::stderr(), "got a frame, size is: {}x{}", frame.width(), frame.height());

                let out_name = format!("{}/{}.jpg", jpeg_out, frame_time);

                // creates a sws context converts from the current pixel
                // format to the specified, without resize
                let mut rgb_converter = frame.converter(format::Pixel::RGB24).unwrap();
                rgb_converter.run(&frame, &mut rgb_frame).unwrap();

                let rgb_buffer = rgb_frame.data(0);

                image::save_buffer(out_name, rgb_buffer, rgb_frame.width(), rgb_frame.height(), image::RGB(8)).unwrap();
            }
        };
    }

}

## About ##

This is a super hacktastic bit of example code for the ffmpeg crate which takes
a video stream and outputs a jpeg per key frame.

There is no sane error handling anywhere (unwraps ahoy!) so if you upset the program in the least it will exit.

## Usage##
`cargo run http://someplace/somestream existing-jpeg-dir`

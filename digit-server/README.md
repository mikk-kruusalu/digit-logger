# Digit Logger Server

The server program can be run simply by using `cargo run`. However, my Synology server can only run Docker containers, so in addition there are the Docker container configuration files as well. Build the image with `docker build -t digit-logger .` and run it with `docker run -p 3000:3000 -v ./data:/usr/src/digit-server/data digit-logger`.

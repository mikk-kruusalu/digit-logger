# Digit Logger

This is a small project that logs my gas meter readings.

Since it is an old school meter with a mechanical dial and is placed in an uncomfortable spot, I created this project to automate the process of logging my gas meter readings. And also, I wanted to try out Rust :)

## ESP32 Camera

The ESP32 camera sits in front of the meter. It wakes up every day and takes a picture of the meter. Then it sends the picture and its battery level to my home server with a POST request.

## Home server

Runs a docker container found in `digit-server`. The container exposes two endpoints, one for uploading the picture and another for reporting the battery level. The picture is directly stored in `data` directory and the battery level is stored in `health.log` file that is in csv format.

## TODO

- [ ] Automatic digit extractor.
- [ ] Automatic reporting at the end of each month.

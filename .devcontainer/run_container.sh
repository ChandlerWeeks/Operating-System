#!/bin/bash

# -it sets up an interactive terminal. --rm emoves the container after exit, -v mounts local drive to the directory
docker run -it --rm -v ../:/home/cosc562/myos cosc562-rust:latest /bin/bash

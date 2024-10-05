FROM docker.io/library/rust:1.80

# Install tdlib; see https://tdlib.github.io/td/build.html?language=Rust
RUN apt-get -y update && apt-get -y upgrade && apt-get -y install make git zlib1g-dev libssl-dev gperf php-cli cmake clang libc++-dev libc++abi-dev locales locales-all

RUN cd /tmp ; git clone https://github.com/tdlib/td.git

WORKDIR /tmp/td

RUN git checkout v1.8.0
RUN rm -rf build
RUN mkdir build

WORKDIR /tmp/td/build

# NB: this step takes a while
RUN CXXFLAGS="-stdlib=libc++" CC=/usr/bin/clang CXX=/usr/bin/clang++ cmake -DCMAKE_BUILD_TYPE=Release -DCMAKE_INSTALL_PREFIX:PATH=/usr ..
RUN cmake --build . -j 16 --target install

WORKDIR /root/src

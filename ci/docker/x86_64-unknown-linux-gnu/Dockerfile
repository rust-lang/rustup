# Source from https://github.com/rust-lang-deprecated/rust-buildbot/blob/master/slaves/dist/Dockerfile
FROM alexcrichton/rust-slave-dist:2015-10-20b
USER root

WORKDIR /
RUN curl https://www.cpan.org/src/5.0/perl-5.28.0.tar.gz | tar xzf -
WORKDIR /perl-5.28.0
RUN ./configure.gnu
RUN make -j$(nproc)
RUN make install

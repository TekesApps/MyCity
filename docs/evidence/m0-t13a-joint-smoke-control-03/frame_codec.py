#!/usr/bin/env python3
"""Lossless independent 4-frame DEFLATE blocks for full-resolution host samples."""
import json
import struct
import zlib

MAGIC = b"MCF4\x00\x01\r\n"
HEADER = struct.Struct("<IIII")  # frame count, raw bytes, compressed bytes, CRC32
BLOCK_FRAMES = 4
MAX_FRAME_RAW_BYTES = 8 * 1024**2
MAX_BLOCK_RAW_BYTES = BLOCK_FRAMES * MAX_FRAME_RAW_BYTES
MAX_PACKED_BYTES = MAX_BLOCK_RAW_BYTES + 5 * ((MAX_BLOCK_RAW_BYTES + 16_383) // 16_384) + 64


class LogCapExceeded(RuntimeError):
    pass


def encode_block(frames):
    frames = list(frames)
    if not 1 <= len(frames) <= BLOCK_FRAMES:
        raise ValueError("block must contain one to four complete frames")
    if any(not isinstance(frame, bytes) or not frame.endswith(b"\n") for frame in frames):
        raise ValueError("each frame must be its original newline-terminated bytes")
    if any(len(frame) > MAX_FRAME_RAW_BYTES for frame in frames):
        raise ValueError("host sample frame exceeds the explicit 8MiB frame bound")
    raw = b"".join(frames)
    if len(raw) > MAX_BLOCK_RAW_BYTES:
        raise ValueError("host sample block exceeds its explicit raw-byte bound")
    packed = zlib.compressobj(level=6, wbits=-zlib.MAX_WBITS)
    compressed = packed.compress(raw) + packed.flush()
    return HEADER.pack(len(frames), len(raw), len(compressed), zlib.crc32(raw)) + compressed


def iter_blocks(stream):
    if stream.read(len(MAGIC)) != MAGIC:
        raise ValueError("bad host-frame stream magic")
    while True:
        header = stream.read(HEADER.size)
        if not header:
            return
        if len(header) != HEADER.size:
            raise ValueError("truncated host-frame block header")
        count, raw_size, packed_size, crc = HEADER.unpack(header)
        if (not 1 <= count <= BLOCK_FRAMES or raw_size == 0 or packed_size == 0
                or raw_size > MAX_BLOCK_RAW_BYTES or raw_size > count * MAX_FRAME_RAW_BYTES
                or packed_size > MAX_PACKED_BYTES):
            raise ValueError("invalid host-frame block dimensions")
        packed = stream.read(packed_size)
        if len(packed) != packed_size:
            raise ValueError("truncated host-frame block payload")
        inflater = zlib.decompressobj(wbits=-zlib.MAX_WBITS)
        # One extra byte is sufficient to detect expansion beyond the advertised
        # raw bound. Never call an unbounded flush on an untrusted block.
        raw = inflater.decompress(packed, raw_size + 1)
        if len(raw) > raw_size or not inflater.eof or inflater.unused_data or inflater.unconsumed_tail:
            raise ValueError("invalid host-frame DEFLATE payload")
        if len(raw) != raw_size or zlib.crc32(raw) != crc:
            raise ValueError("host-frame block length or CRC mismatch")
        frames = raw.splitlines(keepends=True)
        if len(frames) != count or any(not f.endswith(b"\n") or len(f) > MAX_FRAME_RAW_BYTES for f in frames):
            raise ValueError("host-frame count/newline mismatch")
        for frame in frames:
            json.loads(frame)
        yield {"encoded_bytes": HEADER.size + packed_size, "raw_bytes": raw_size, "frames": frames}


class FrameWriter:
    def __init__(self, stream, cap_bytes, latency_bytes_provider):
        self.stream = stream
        self.cap_bytes = cap_bytes
        self.latency_bytes_provider = latency_bytes_provider
        self.pending = []
        self.block_count = 0
        self.max_encoded_block_bytes = 0
        self.max_raw_block_bytes = 0
        self.persisted_frame_count = 0
        self.persisted_raw_bytes = 0
        self._admit(len(MAGIC))
        self.stream.write(MAGIC)

    def add(self, frame):
        candidate_frames = self.pending + [frame]
        encoded = encode_block(candidate_frames)
        self._admit(len(encoded))
        if len(candidate_frames) == BLOCK_FRAMES:
            self._write_encoded(encoded, candidate_frames)
            self.pending.clear()
        else:
            self.pending.append(frame)

    def flush(self):
        if not self.pending:
            return
        encoded = encode_block(self.pending)
        self._write_encoded(encoded, self.pending)
        self.pending.clear()

    def _admit(self, candidate_encoded_bytes):
        current_latency_bytes = self.latency_bytes_provider()
        committed_host_bytes = self.stream.tell()
        projected = committed_host_bytes + candidate_encoded_bytes + current_latency_bytes
        if projected > self.cap_bytes:
            raise LogCapExceeded(
                "combined measurement-log cap: committed_host_bytes=" + str(committed_host_bytes)
                + " candidate_block_bytes=" + str(candidate_encoded_bytes)
                + " compute_duration_bytes=" + str(current_latency_bytes)
                + " cap_bytes=" + str(self.cap_bytes)
            )

    def _write_encoded(self, encoded, frames):
        # Re-read the latest latency size immediately before every actual write.
        self._admit(len(encoded))
        raw_bytes = sum(map(len, frames))
        self.stream.write(encoded)
        self.stream.flush()
        self.block_count += 1
        self.max_encoded_block_bytes = max(self.max_encoded_block_bytes, len(encoded))
        self.max_raw_block_bytes = max(self.max_raw_block_bytes, raw_bytes)
        self.persisted_frame_count += len(frames)
        self.persisted_raw_bytes += raw_bytes

    def discard_pending(self):
        count = len(self.pending)
        raw_bytes = sum(map(len, self.pending))
        self.pending.clear()
        return count, raw_bytes


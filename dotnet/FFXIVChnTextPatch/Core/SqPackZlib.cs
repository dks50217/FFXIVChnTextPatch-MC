using System.IO.Compression;

namespace FFXIVChnTextPatch.Core;

/// <summary>SqPack 區塊的 raw deflate 壓縮/解壓。取代 Java 版 jzlib（Java 版手動補 zlib header；DeflateStream 是 raw deflate，直接處理即可）。</summary>
public static class SqPackZlib
{
    public static byte[] Compress(byte[] block)
    {
        using var output = new MemoryStream();
        using (var deflate = new DeflateStream(output, CompressionLevel.SmallestSize, leaveOpen: true))
            deflate.Write(block, 0, block.Length);
        return output.ToArray();
    }

    public static byte[] Decompress(byte[] compressed, int decompressedSize)
    {
        var result = new byte[decompressedSize];
        using var input = new MemoryStream(compressed);
        using var deflate = new DeflateStream(input, CompressionMode.Decompress);
        deflate.ReadExactly(result, 0, decompressedSize);
        return result;
    }
}

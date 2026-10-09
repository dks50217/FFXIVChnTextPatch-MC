using System.Buffers.Binary;

namespace FFXIVChnTextPatch.Core;

// 注意：EXH/EXD 內容是 big-endian（與 SqPack index/dat 的 little-endian 相反）。

public record struct EXDFDataset(short Type, short Offset);

public record struct EXDFPage(int PageNum, int NumEntries);

/// <summary>EXH 標頭檔。移植自 EXHFFile.java。</summary>
public class EXHFFile
{
    public int DatasetChunkSize { get; private set; }
    public EXDFDataset[] Datasets { get; private set; } = Array.Empty<EXDFDataset>();
    public EXDFPage[] Pages { get; private set; } = Array.Empty<EXDFPage>();
    public int[] Langs { get; private set; } = Array.Empty<int>();

    public EXHFFile(byte[] data)
    {
        int magic = BinaryPrimitives.ReadInt32BigEndian(data);
        if (magic != 0x45584846) // "EXHF"
            throw new IOException("Not a EXHF");
        short version = BinaryPrimitives.ReadInt16BigEndian(data.AsSpan(4));
        if (version != 3)
            throw new IOException("Not a EXHF");

        DatasetChunkSize = BinaryPrimitives.ReadInt16BigEndian(data.AsSpan(6));
        int numDatasets = BinaryPrimitives.ReadInt16BigEndian(data.AsSpan(8));
        int numPages = BinaryPrimitives.ReadInt16BigEndian(data.AsSpan(10));
        int numLangs = BinaryPrimitives.ReadInt16BigEndian(data.AsSpan(12));

        try
        {
            int pos = 32;
            Datasets = new EXDFDataset[numDatasets];
            for (int i = 0; i < numDatasets; i++)
            {
                Datasets[i] = new EXDFDataset(
                    BinaryPrimitives.ReadInt16BigEndian(data.AsSpan(pos)),
                    BinaryPrimitives.ReadInt16BigEndian(data.AsSpan(pos + 2)));
                pos += 4;
            }
            Pages = new EXDFPage[numPages];
            for (int i = 0; i < numPages; i++)
            {
                Pages[i] = new EXDFPage(
                    BinaryPrimitives.ReadInt32BigEndian(data.AsSpan(pos)),
                    BinaryPrimitives.ReadInt32BigEndian(data.AsSpan(pos + 4)));
                pos += 8;
            }
            Langs = new int[numLangs];
            for (int i = 0; i < numLangs; i++)
            {
                Langs[i] = data[pos];
                pos += 2;
            }
        }
        catch (ArgumentOutOfRangeException)
        {
            // Java 版對 BufferUnderflow 同樣靜默容忍
        }
    }
}

/// <summary>EXD 資料檔。移植自 EXDFFile.java。</summary>
public class EXDFFile
{
    public Dictionary<int, byte[]> Entries { get; } = new();

    public EXDFFile(byte[] data)
    {
        int magic = BinaryPrimitives.ReadInt32BigEndian(data);
        short version = BinaryPrimitives.ReadInt16BigEndian(data.AsSpan(4));
        if (magic != 0x45584446 || version != 2) // "EXDF"
            throw new IOException("Not a EXDF");
        int offsetTableSize = BinaryPrimitives.ReadInt32BigEndian(data.AsSpan(8));
        int pos = 32;
        for (int i = 0; i < offsetTableSize / 8; i++)
        {
            int index = BinaryPrimitives.ReadInt32BigEndian(data.AsSpan(pos));
            int offset = BinaryPrimitives.ReadInt32BigEndian(data.AsSpan(pos + 4));
            pos += 8;
            int size = BinaryPrimitives.ReadInt32BigEndian(data.AsSpan(offset));
            var entry = new byte[size];
            Array.Copy(data, offset + 6, entry, 0, size); // +4 size +2 flags
            Entries[index] = entry;
        }
    }
}

/// <summary>EXD 單列資料（chunk = 固定欄位區、string = 字串區）。移植自 EXDFEntry.java。</summary>
public class EXDFEntry
{
    public byte[] Chunk { get; }
    public byte[] Data { get; }

    public EXDFEntry(byte[] data, int datasetChunkSize)
    {
        if (data.Length < datasetChunkSize)
        {
            AppEnv.Log($"EXDFEntry: data length {data.Length} < chunk size {datasetChunkSize}");
            Chunk = Array.Empty<byte>();
            Data = Array.Empty<byte>();
            return;
        }
        Chunk = data[..datasetChunkSize];
        Data = data;
    }

    /// <summary>讀出 offset 欄位指到的 null 結尾字串（不含結尾 0）。</summary>
    public byte[] GetString(short offset)
    {
        int datasetChunkSize = Chunk.Length;
        int stringOffset = BinaryPrimitives.ReadInt32BigEndian(Data.AsSpan(offset));
        int start = datasetChunkSize + stringOffset;
        if (start >= Data.Length)
            return Array.Empty<byte>();
        int end = start;
        while (Data[end] != 0) end++;
        return Data[start..end];
    }
}

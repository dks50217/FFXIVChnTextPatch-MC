namespace FFXIVChnTextPatch.Core;

public record SqPackIndexFile(long Pt, int Id, int Id2, long DataOffset);

public class SqPackIndexFolder
{
    private readonly int _numFiles;
    private readonly long _fileIndexOffset;

    public Dictionary<int, SqPackIndexFile> Files { get; } = new();

    public SqPackIndexFolder(int numFiles, long fileIndexOffset)
    {
        _numFiles = numFiles;
        _fileIndexOffset = fileIndexOffset;
    }

    public void ReadFiles(FileStream fs, BinaryReader r, bool isIndex2)
    {
        fs.Seek(_fileIndexOffset, SeekOrigin.Begin);
        for (int i = 0; i < _numFiles; i++)
        {
            long pt = fs.Position;
            if (!isIndex2)
            {
                int id = r.ReadInt32();
                int id2 = r.ReadInt32();
                long dataOffset = r.ReadInt32();
                r.ReadInt32();
                Files[id] = new SqPackIndexFile(pt, id, id2, dataOffset);
            }
            else
            {
                int id = r.ReadInt32();
                long dataOffset = r.ReadInt32();
                Files[id] = new SqPackIndexFile(pt, id, -1, dataOffset);
            }
        }
    }
}

public class SqPackIndex
{
    private readonly string _indexPath;

    public SqPackIndex(string indexPath) => _indexPath = indexPath;

    public Dictionary<int, SqPackIndexFolder> ResolveIndex()
    {
        using var fs = File.OpenRead(_indexPath);
        using var r = new BinaryReader(fs);
        int sqpackHeaderLength = CheckSqPackHeader(fs, r);
        var segments = ResolveSegments(fs, r, sqpackHeaderLength);
        var indexMap = new Dictionary<int, SqPackIndexFolder>();
        // Segment 1 是檔案、Segment 4 是資料夾
        if (segments[3] != null && segments[3].Offset != 0)
        {
            int offset = segments[3].Offset;
            int numFolders = segments[3].Size / 16;
            for (int i = 0; i < numFolders; i++)
            {
                fs.Seek(offset + i * 16L, SeekOrigin.Begin);
                int id = r.ReadInt32();
                int fileIndexOffset = r.ReadInt32();
                int folderSize = r.ReadInt32();
                int numFiles = folderSize / 16;
                r.ReadInt32();
                var folder = new SqPackIndexFolder(numFiles, fileIndexOffset);
                indexMap[id] = folder;
                folder.ReadFiles(fs, r, isIndex2: false);
            }
        }
        else
        {
            bool isIndex2 = _indexPath.Contains("index2");
            int numFiles = (isIndex2 ? 2 : 1) * segments[0].Size / 16;
            var folder = new SqPackIndexFolder(numFiles, segments[0].Offset);
            indexMap[0] = folder;
            folder.ReadFiles(fs, r, isIndex2);
        }
        return indexMap;
    }

    private static int CheckSqPackHeader(FileStream fs, BinaryReader r)
    {
        var buffer = r.ReadBytes(6);
        if (buffer.Length != 6 || buffer[0] != 0x53 || buffer[1] != 0x71 || buffer[2] != 0x50
            || buffer[3] != 0x61 || buffer[4] != 0x63 || buffer[5] != 0x6B)
            throw new IOException("Not a SqPack file");
        fs.Seek(0xC, SeekOrigin.Begin);
        int headerLength = r.ReadInt32();
        r.ReadInt32();
        int type = r.ReadInt32();
        if (type != 2)
            throw new IOException("Not a index");
        return headerLength;
    }

    private record Segment(int Offset, int Size);

    private static Segment[] ResolveSegments(FileStream fs, BinaryReader r, int segmentHeaderStart)
    {
        fs.Seek(segmentHeaderStart, SeekOrigin.Begin);
        r.ReadInt32(); // header length
        var segments = new Segment[4];
        for (int i = 0; i < segments.Length; i++)
        {
            r.ReadInt32(); // segment 2 是 dat 檔數量，其他未知
            int offset = r.ReadInt32();
            int size = r.ReadInt32();
            r.ReadBytes(20); // sha1
            segments[i] = new Segment(offset, size);
            if (i == 0)
                fs.Seek(4, SeekOrigin.Current);
            fs.Seek(40, SeekOrigin.Current);
        }
        return segments;
    }
}

public class SqPackDatFile : IDisposable
{
    private readonly FileStream _fs;
    private readonly BinaryReader _r;

    public SqPackDatFile(string path)
    {
        _fs = File.OpenRead(path);
        _r = new BinaryReader(_fs);
    }

    public byte[] ExtractFile(long fileOffset)
    {
        _fs.Seek(fileOffset, SeekOrigin.Begin);
        int headerLength = _r.ReadInt32();
        int contentType = _r.ReadInt32();
        int fileSize = _r.ReadInt32();
        _r.ReadInt32();
        _r.ReadInt32(); // block buffer size (*128)
        int blockCount = _r.ReadInt32();
        if (contentType != 2)
            // ponytail: 漢化流程只會解 EXH/EXD/root.exl（type 2）；type 3/4 需要時再從 SqPackDatFile.java 移植
            throw new NotSupportedException($"SqPack content type {contentType} not supported (offset {fileOffset:X})");

        var blocks = new (int Offset, int DecompressedSize)[blockCount];
        for (int i = 0; i < blockCount; i++)
        {
            int offset = _r.ReadInt32();
            int paddingAndSize = _r.ReadInt32();
            blocks[i] = (offset, (paddingAndSize >> 16) & 0xFFFF);
        }

        var decompressedFile = new byte[fileSize];
        int filePos = 0;
        foreach (var block in blocks)
        {
            _fs.Seek(fileOffset + headerLength + block.Offset, SeekOrigin.Begin);
            _r.ReadInt32(); // block header length
            _r.ReadInt32();
            int compressedSize = _r.ReadInt32();
            int decompressedSize = _r.ReadInt32();
            byte[] decompressed = compressedSize == 32000 || decompressedSize == 1
                ? _r.ReadBytes(decompressedSize)                                    // 未壓縮區塊
                : SqPackZlib.Decompress(_r.ReadBytes(compressedSize), decompressedSize);
            Array.Copy(decompressed, 0, decompressedFile, filePos, decompressedSize);
            filePos += decompressedSize;
        }
        return decompressedFile;
    }

    public void Dispose()
    {
        _r.Dispose();
        _fs.Dispose();
    }
}

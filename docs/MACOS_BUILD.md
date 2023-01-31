# 使用 macOS 版本

原本的 binary 只有 windows 版，如果使用 macOS 可以參考下列步驟把漢化器跑起來

## 準備
專案本身 build.gradle 中不包含， `com.univocity.parsers.csv` 這個 package 需要 [下載](https://oss.sonatype.org/service/local/repositories/releases/content/com/univocity/univocity-parsers/2.9.1/univocity-parsers-2.9.1.jar) ，並放到 `專案目錄/libs` 中。

打開 build.gradle 編輯下列區塊

```gradle
dependencies {
	testCompile(
        'junit:junit:4.12'
    )
	compile("com.jcraft:jzlib:1.1.3")
	compile("org.nlpcn:nlp-lang:1.7.6")
	compile("com.google.code.gson:gson:2.8.5")
	compile("commons-codec:commons-codec:1.11")
	compile("org.apache.httpcomponents:httpcore:4.4.6")
	compile("org.apache.httpcomponents:httpclient:4.5.3")
	compile("org.apache.httpcomponents:httpclient-cache:4.5.3")
	compile("org.apache.httpcomponents:httpmime:4.5.3")
	compile("org.apache.httpcomponents:fluent-hc:4.5.3")
	compile(files("libs/univocity-parsers-2.9.1.jar"))  // 增加這行
}
```
又或者修改為：
```
compile group: 'com.univocity', name: 'univocity-parsers', version: '2.9.1' // 修改成這行可以不用另外下載jar
```

> 推薦使用 [asdf](https://github.com/asdf-vm/asdf) 管理下列工具版本

這個專案需要 jdk 11 ，可以透過下列方式安裝，無法使用下列指令的話，請先安裝 [Homebrew](https://brew.sh/index_zh-tw)

```shell
brew install openjdk@11
```

> 每次 run 之前先跑一次下列指令 [^1]

```shell
export PATH="/usr/local/opt/openjdk@11/bin:$PATH"
```
在Mac M1 的情況下，可用brew info 查詢
```shell 
brew info openjdk@11 
```
例如我查詢到的路徑就是在
```shell
export PATH="/opt/homebrew/opt/openjdk@11/bin:$PATH"
```

然後會用到 gradle 6.x.x 的版本，[下載](https://gradle.org/next-steps/?version=6.8.3&format=bin) 後解壓縮，把資料夾中 bin 的路徑加入環境變數 PATH 中

> 每次 run 之前先跑一次下列指令 [^1]

```shell
export PATH="/Users/xyz/Downloads/gradle-6.8.3/bin:$PATH"
```


*在目前版本的程式碼已經改使用HashMap，所以不會遇到該問題，所以以下的步驟可省略，直接到Run的部分。
***
因為 build 的時候會遇到 `cannot find symbol` 的問題，需要修改一下程式碼。

位置 `src/main/java/name/yumao/ffxiv/chn/replace/ReplaceFont.java` 第 `86` 行

```java
var rootIndexFileSE = ((SqPackIndexFolder)indexSE.get(filePathCRC)).getFiles();
```

改成

```java
HashMap<Integer, SqPackIndexFile> rootIndexFileSE = ((SqPackIndexFolder)indexSE.get(filePathCRC)).getFiles();
```

## Run

在 Terminal 中，執行

```shell
gradle run
```

## 註解

[^1] 也可以將指令放入 .bashrc 或 .zshrc 就可以不用每次都執行 (加入後需要重新啟動 Termial 才會生效)

## One more thing
使用XIV on Mac的朋友們，FFXIV的根目錄可以在左上的File-> Open Install Folder 找到，
第一次選擇會跳出錯誤視窗，提示你正確的folder名稱，不要被嚇到，再選一次，按下漢化就好。


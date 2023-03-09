# 使用 macOS 版本

原本的 binary 只有 windows 版，如果使用 macOS 可以參考下列步驟把漢化器跑起來

## 準備環境

> 推薦使用 [asdf](https://github.com/asdf-vm/asdf) 管理下列工具版本

### JDK

這個專案需要 jdk 11 ，推薦使用 [Homebrew](https://brew.sh/index_zh-tw) 進行安裝。

```shell
brew install openjdk@11
```

### Gradle

編譯這個專案需要 gradle 6.x.x 的版本，可以[透過此連結下載](https://gradle.org/next-steps/?version=6.8.3&format=bin)。
請記得下載的路徑，並將解壓縮後資料夾裡的 `bin` 路徑加入環境變數 `PATH` 中。

### 其他函式庫 (csv parser)

專案本身 build.gradle 中不包含， `com.univocity.parsers.csv` 這個 package 需要 [下載](https://oss.sonatype.org/service/local/repositories/releases/content/com/univocity/univocity-parsers/2.9.1/univocity-parsers-2.9.1.jar) ，並放到 `專案目錄/libs` 中。

**如不想下載此函式庫**，打開 `build.gradle` 編輯下列區塊

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
	compile(files("libs/univocity-parsers-2.9.1.jar"))  // 刪除這行
	compile group: 'com.univocity', name: 'univocity-parsers', version: '2.9.1' // 修改成這行可以不用另外下載 jar
}
```

## 執行

### 設定環境變數

> 每次 run 之前先跑一次下列指令 [^1]

#### 設定 openjdk 的路徑

##### Mac (Intel)

```shell
export PATH="/usr/local/opt/openjdk@11/bin:$PATH"
```

##### Mac (Apple Silicon)

```shell
export PATH="/opt/homebrew/opt/openjdk@11/bin:$PATH"
```

##### 查詢 openjdk 安裝路徑

如你不確定所使用的 mac 硬體，或相關的路徑資訊，可用 `brew info` 進行查詢。

```shell
brew info openjdk@11
```

#### 設定 gradle 的路徑

請將下面的 `/Users/xyz/Downloads/gradle-6.8.3` 改成你放置 gradle 解壓縮後的資料夾路徑。

```shell
export PATH="/Users/xyz/Downloads/gradle-6.8.3/bin:$PATH"
```

### 執行程式

在 Terminal 中，執行

```shell
gradle run
```

## 註解

[^1] 也可以將指令放入 .bashrc 或 .zshrc 就可以不用每次都執行 (加入後需要重新啟動 Termial 才會生效)

## One more thing

使用 XIV on Mac 的朋友們，FFXIV 的根目錄可以在左上的 File-> Open Install Folder 找到，
第一次選擇會跳出錯誤視窗，提示你正確的 folder 名稱，不要被嚇到，再選一次，按下漢化就好。

路徑的結尾為 ffxiv ，比如說 `/Users/使用者帳號/Library/Application Support/XIV on Mac/ffxiv`

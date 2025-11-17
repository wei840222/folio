import { useState } from "react";
import { Card, CardContent, CardHeader, CardTitle } from "@/components/ui/card";
import { Button } from "@/components/ui/button";
import { Upload, Link, Copy, Download } from "lucide-react";
import FileUploadZone from "@/components/FileUploadZone";
import DownloadLink from "@/components/DownloadLink";
import "./App.css";

function App() {
  const [uploadedFile, setUploadedFile] = useState<File | null>(null);
  const [isUploading, setIsUploading] = useState(false);
  const [shortUrl, setShortUrl] = useState<string>("");

  const handleFileUpload = async (file: File) => {
    setIsUploading(true);

    const formData = new FormData();
    formData.append("file", file);

    try {
      const res = await fetch("/uploads", {
        method: "POST",
        body: formData,
      });
      if (!res.ok) {
        throw new Error("上傳失敗");
      }
      const data = await res.json();
      setShortUrl(`${window.location.origin}${data.path}`);
    } catch (error) {
      console.error("上傳錯誤:", error);
      alert("檔案上傳失敗，請稍後再試。");
      return;
    } finally {
      setIsUploading(false);
      setUploadedFile(file);
    }
  };

  const handleReset = () => {
    setUploadedFile(null);
    setShortUrl("");
  };

  return (
    <div className="min-h-screen bg-gradient-to-br from-blue-50 via-white to-purple-50">
      <div className="container mx-auto px-4 py-8 flex flex-col justify-center min-h-screen">
        <div className="max-w-2xl mx-auto">
          {/* Header */}
          <div className="text-center mb-8">
            <h1 className="text-4xl font-bold text-gray-900 mb-4">Folio</h1>
            <p className="text-lg text-gray-600">
              上傳您的檔案，生成短網址，輕鬆分享給朋友或同事。
            </p>
          </div>

          {/* Main Card */}
          <Card className="shadow-xl border-0 bg-white/80 backdrop-blur-sm">
            <CardHeader className="text-center pb-6">
              <CardTitle className="flex items-center justify-center gap-2 text-2xl">
                <Upload className="w-6 h-6 text-blue-600" />
                檔案上傳
              </CardTitle>
            </CardHeader>
            <CardContent className="space-y-6">
              {!uploadedFile ? (
                <FileUploadZone
                  onFileSelect={handleFileUpload}
                  isUploading={isUploading}
                />
              ) : (
                <div className="space-y-6">
                  {/* Upload Success Info */}
                  <div className="bg-green-50 border border-green-200 rounded-lg p-4">
                    <div className="flex items-center gap-3">
                      <div className="w-10 h-10 bg-green-100 rounded-full flex items-center justify-center">
                        <Download className="w-5 h-5 text-green-600" />
                      </div>
                      <div>
                        <h3 className="font-semibold text-green-800">
                          {uploadedFile.name}
                        </h3>
                        <p className="text-sm text-green-600">
                          檔案大小：
                          {(uploadedFile.size / 1024 / 1024).toFixed(2)} MB
                        </p>
                      </div>
                    </div>
                  </div>

                  {/* Download Link */}
                  {shortUrl && (
                    <DownloadLink url={shortUrl} fileName={uploadedFile.name} />
                  )}

                  {/* Reset Button */}
                  <Button
                    onClick={handleReset}
                    variant="outline"
                    className="w-full"
                  >
                    上傳新檔案
                  </Button>
                </div>
              )}
            </CardContent>
          </Card>

          {/* Features */}
          <div className="grid md:grid-cols-3 gap-6 mt-12">
            <div className="text-center">
              <div className="w-12 h-12 bg-blue-100 rounded-lg flex items-center justify-center mx-auto mb-3">
                <Upload className="w-6 h-6 text-blue-600" />
              </div>
              <h3 className="font-semibold mb-2">快速上傳</h3>
              <p className="text-sm text-gray-600">支援拖曳上傳，方便快速。</p>
            </div>
            <div className="text-center">
              <div className="w-12 h-12 bg-purple-100 rounded-lg flex items-center justify-center mx-auto mb-3">
                <Link className="w-6 h-6 text-purple-600" />
              </div>
              <h3 className="font-semibold mb-2">短網址生成</h3>
              <p className="text-sm text-gray-600">
                自動生成方便分享的短網址。
              </p>
            </div>
            <div className="text-center">
              <div className="w-12 h-12 bg-green-100 rounded-lg flex items-center justify-center mx-auto mb-3">
                <Copy className="w-6 h-6 text-green-600" />
              </div>
              <h3 className="font-semibold mb-2">一鍵複製</h3>
              <p className="text-sm text-gray-600">點選就能複製分享連結。</p>
            </div>
          </div>
        </div>
      </div>
    </div>
  );
}

export default App;

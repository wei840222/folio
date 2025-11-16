import React, { useCallback, useState } from "react";
import { Button } from "@/components/ui/button";
import { Upload, FileText } from "lucide-react";

interface FileUploadZoneProps {
  onFileSelect: (file: File) => void;
  isUploading: boolean;
}

const FileUploadZone: React.FC<FileUploadZoneProps> = ({
  onFileSelect,
  isUploading,
}) => {
  const [isDragOver, setIsDragOver] = useState(false);

  const handleDragOver = useCallback((e: React.DragEvent) => {
    e.preventDefault();
    setIsDragOver(true);
  }, []);

  const handleDragLeave = useCallback((e: React.DragEvent) => {
    e.preventDefault();
    setIsDragOver(false);
  }, []);

  const handleDrop = useCallback(
    (e: React.DragEvent) => {
      e.preventDefault();
      setIsDragOver(false);

      const files = Array.from(e.dataTransfer.files);
      if (files.length > 0) {
        onFileSelect(files[0]);
      }
    },
    [onFileSelect]
  );

  const handleFileInput = (e: React.ChangeEvent<HTMLInputElement>) => {
    const files = e.target.files;
    if (files && files.length > 0) {
      onFileSelect(files[0]);
    }
  };

  if (isUploading) {
    return (
      <div className="border-2 border-dashed border-blue-300 rounded-lg p-12 text-center bg-blue-50">
        <div className="animate-spin w-8 h-8 border-3 border-blue-600 border-t-transparent rounded-full mx-auto mb-4"></div>
        <h3 className="text-lg font-semibold text-blue-800 mb-2">上傳中...</h3>
        <p className="text-blue-600">請稍後，檔案處理中...</p>
      </div>
    );
  }

  return (
    <div
      className={`border-2 border-dashed rounded-lg p-12 text-center transition-all duration-200 cursor-pointer ${
        isDragOver
          ? "border-blue-500 bg-blue-50 scale-105"
          : "border-gray-300 hover:border-blue-400 hover:bg-gray-50"
      }`}
      onDragOver={handleDragOver}
      onDragLeave={handleDragLeave}
      onDrop={handleDrop}
      onClick={() => document.getElementById("file-input")?.click()}
    >
      <div className="space-y-4">
        <div className="mx-auto w-16 h-16 bg-blue-100 rounded-full flex items-center justify-center">
          <Upload className="w-8 h-8 text-blue-600" />
        </div>

        <div>
          <h3 className="text-lg font-semibold text-gray-900 mb-2">
            拖曳檔案到此處或點選上傳按鈕
          </h3>
          <p className="text-gray-600 mb-4">支援所有檔案類型，最大大小5MB</p>
        </div>

        <Button type="button" className="bg-blue-600 hover:bg-blue-700">
          <FileText className="w-4 h-4 mr-2" />
          選擇檔案
        </Button>
      </div>

      <input
        id="file-input"
        type="file"
        className="hidden"
        onChange={handleFileInput}
        accept="*/*"
      />
    </div>
  );
};

export default FileUploadZone;

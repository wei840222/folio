import React, { useState } from "react";
import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import { Label } from "@/components/ui/label";
import { Copy, Link, ExternalLink } from "lucide-react";

interface DownloadLinkProps {
  url: string;
  fileName: string;
}

const DownloadLink: React.FC<DownloadLinkProps> = ({ url }) => {
  const [copied, setCopied] = useState(false);

  const handleCopy = async () => {
    await navigator.clipboard.writeText(url);
    setCopied(true);
    setTimeout(() => setCopied(false), 2000);
  };

  return (
    <div className="bg-blue-50 border border-blue-200 rounded-lg p-6">
      <div className="flex items-center gap-3 mb-4">
        <div className="w-10 h-10 bg-blue-100 rounded-full flex items-center justify-center">
          <Link className="w-5 h-5 text-blue-600" />
        </div>
        <div>
          <h3 className="font-semibold text-blue-800">分享連結已生成</h3>
          <p className="text-sm text-blue-600">您可以透過連結分享檔案。</p>
        </div>
      </div>

      <div className="space-y-3">
        <div>
          <Label
            htmlFor="download-url"
            className="text-sm font-medium text-gray-700"
          >
            分享連結
          </Label>
          <div className="flex gap-2 mt-1">
            <Input
              id="download-url"
              type="url"
              value={url}
              readOnly
              className="bg-white"
            />
            <Button
              onClick={handleCopy}
              variant={copied ? "secondary" : "outline"}
              size="sm"
              className="shrink-0"
            >
              {copied ? (
                <>
                  <Copy className="w-4 h-4 mr-1" />
                  已複製
                </>
              ) : (
                <>
                  <Copy className="w-4 h-4 mr-1" />
                  複製
                </>
              )}
            </Button>
          </div>
        </div>

        <Button
          onClick={() => window.open(url, "_blank")}
          className="w-full bg-blue-600 hover:bg-blue-700"
        >
          <ExternalLink className="w-4 h-4 mr-2" />
          打開連結
        </Button>
      </div>

      <div className="mt-4 pt-4 border-t border-blue-200">
        <p className="text-xs text-blue-600">連結有效時間：7天</p>
      </div>
    </div>
  );
};

export default DownloadLink;

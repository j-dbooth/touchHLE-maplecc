/*
 * This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/.
 *
 * Parts of this file are derived from SDL 2's Android project template, which
 * has a different license. Please see vendor/SDL/LICENSE.txt for details.
 */
package org.touchhle.android;

import android.content.res.AssetManager;
import android.os.Bundle;
import android.util.Log;
import org.libsdl.app.SDLActivity;

import java.io.File;
import java.io.FileOutputStream;
import java.io.IOException;
import java.io.InputStream;
import java.io.OutputStream;
import java.util.zip.ZipEntry;
import java.util.zip.ZipInputStream;

/**
 * Dedicated launcher Activity for MapleStory: Cave Crawlers
 */
public class MainActivity extends SDLActivity {
    private static final String TAG = "MapleCC";
    private static final String APP_BUNDLE_NAME = "MapleCC.app";
    private static final String APPS_DIR_NAME = "touchHLE_apps";

    @Override
    protected void onCreate(Bundle savedInstanceState) {
        setupGameBundle();
        super.onCreate(savedInstanceState);
    }

    @Override
    protected String[] getLibraries() {
        return new String[]{
            "SDL2",
            "touchHLE"
        };
    }

    @Override
    protected String[] getArguments() {
        File extDir = getExternalFilesDir(null);
        if (extDir != null) {
            File gameApp = new File(new File(extDir, APPS_DIR_NAME), APP_BUNDLE_NAME);
            if (gameApp.exists()) {
                Log.i(TAG, "Passing auto-boot argument for MapleCC: " + gameApp.getAbsolutePath());
                return new String[]{ gameApp.getAbsolutePath() };
            }
            File gameIpa = new File(new File(extDir, APPS_DIR_NAME), "MapleCC.ipa");
            if (gameIpa.exists()) {
                Log.i(TAG, "Passing auto-boot argument for MapleCC IPA: " + gameIpa.getAbsolutePath());
                return new String[]{ gameIpa.getAbsolutePath() };
            }
        }
        return new String[0];
    }

    /**
     * Unpacks the bundled game assets to external storage if not already present.
     */
    private void setupGameBundle() {
        File extDir = getExternalFilesDir(null);
        if (extDir == null) {
            Log.e(TAG, "External files directory is not available");
            return;
        }

        File appsDir = new File(extDir, APPS_DIR_NAME);
        if (!appsDir.exists() && !appsDir.mkdirs()) {
            Log.e(TAG, "Failed to create apps directory: " + appsDir.getAbsolutePath());
        }

        File targetAppDir = new File(appsDir, APP_BUNDLE_NAME);
        File marker = new File(targetAppDir, "Info.plist");
        if (marker.exists()) {
            Log.i(TAG, "MapleCC.app already installed at " + targetAppDir.getAbsolutePath());
            return;
        }

        AssetManager am = getAssets();

        // 1. Try extracting bundled zip (MapleCC.zip or touchHLE_apps/MapleCC.zip)
        String[] zipCandidates = new String[] {
            "MapleCC.zip",
            APPS_DIR_NAME + "/MapleCC.zip"
        };
        for (String zipPath : zipCandidates) {
            try (InputStream is = am.open(zipPath)) {
                Log.i(TAG, "Extracting bundled game archive: " + zipPath);
                unzip(is, appsDir);
                if (marker.exists()) {
                    Log.i(TAG, "Successfully extracted " + APP_BUNDLE_NAME + " from " + zipPath);
                    return;
                }
            } catch (IOException ignored) {
                // Not present under this path
            }
        }

        // 2. Try copying single IPA package if bundled
        String[] ipaCandidates = new String[] {
            APPS_DIR_NAME + "/MapleCC.ipa",
            APPS_DIR_NAME + "/MapleStory_Cave_Crawlers_1.0.1.235.ipa",
            "MapleCC.ipa"
        };
        for (String ipaPath : ipaCandidates) {
            try (InputStream is = am.open(ipaPath)) {
                File targetIpa = new File(appsDir, "MapleCC.ipa");
                Log.i(TAG, "Copying bundled game IPA to " + targetIpa.getAbsolutePath());
                copyStream(is, targetIpa);
                return;
            } catch (IOException ignored) {
                // Not present under this path
            }
        }

        // 3. Try copying loose files from assets directory (touchHLE_apps/MapleCC.app)
        String assetAppPath = APPS_DIR_NAME + "/" + APP_BUNDLE_NAME;
        try {
            String[] list = am.list(assetAppPath);
            if (list != null && list.length > 0) {
                Log.i(TAG, "Copying loose bundled app directory from assets: " + assetAppPath);
                copyAssetFolder(am, assetAppPath, targetAppDir);
                Log.i(TAG, "Finished copying bundled app directory");
            }
        } catch (IOException e) {
            Log.e(TAG, "Failed reading asset folder " + assetAppPath + ": " + e.getMessage());
        }
    }

    private void unzip(InputStream is, File targetDir) throws IOException {
        byte[] buffer = new byte[65536];
        try (ZipInputStream zis = new ZipInputStream(is)) {
            ZipEntry entry;
            while ((entry = zis.getNextEntry()) != null) {
                String entryName = entry.getName();
                // Strip optional leading root directory in archive if present
                if (entryName.startsWith("Payload/")) {
                    entryName = entryName.substring("Payload/".length());
                }
                File outFile = new File(targetDir, entryName);
                if (entry.isDirectory()) {
                    if (!outFile.isDirectory() && !outFile.mkdirs()) {
                        throw new IOException("Failed to create directory: " + outFile.getAbsolutePath());
                    }
                } else {
                    File parent = outFile.getParentFile();
                    if (parent != null && !parent.exists() && !parent.mkdirs()) {
                        throw new IOException("Failed to create parent directory: " + parent.getAbsolutePath());
                    }
                    try (FileOutputStream fos = new FileOutputStream(outFile)) {
                        int len;
                        while ((len = zis.read(buffer)) > 0) {
                            fos.write(buffer, 0, len);
                        }
                    }
                }
                zis.closeEntry();
            }
        }
    }

    private void copyAssetFolder(AssetManager am, String srcPath, File dstDir) throws IOException {
        String[] files = am.list(srcPath);
        if (files == null || files.length == 0) {
            // It's a file
            try (InputStream is = am.open(srcPath)) {
                copyStream(is, dstDir);
            }
        } else {
            // It's a directory
            if (!dstDir.exists() && !dstDir.mkdirs()) {
                throw new IOException("Failed to create directory: " + dstDir.getAbsolutePath());
            }
            for (String file : files) {
                String subSrc = srcPath + "/" + file;
                File subDst = new File(dstDir, file);
                copyAssetFolder(am, subSrc, subDst);
            }
        }
    }

    private void copyStream(InputStream is, File dst) throws IOException {
        byte[] buffer = new byte[65536];
        try (FileOutputStream fos = new FileOutputStream(dst)) {
            int len;
            while ((len = is.read(buffer)) > 0) {
                fos.write(buffer, 0, len);
            }
        }
    }
}

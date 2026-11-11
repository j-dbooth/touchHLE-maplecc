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
import java.util.Enumeration;
import java.util.zip.ZipEntry;
import java.util.zip.ZipFile;
import java.util.zip.ZipInputStream;

/**
 * Dedicated standalone launcher Activity for MapleStory: Cave Crawlers.
 */
public class MainActivity extends SDLActivity {
    private static final String TAG = "MapleCC";
    private static final String APP_BUNDLE_NAME = "MapleCC.app";
    private static final String APPS_DIR_NAME = "touchHLE_apps";
    private static final String INSTALL_MARKER = ".installed";

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
     * Uses fast direct APK ZIP extraction to prevent Android UI thread ANR timeouts.
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
        File marker = new File(targetAppDir, INSTALL_MARKER);
        File infoPlist = new File(targetAppDir, "Info.plist");

        if (marker.exists() && infoPlist.exists()) {
            Log.i(TAG, "MapleCC.app already installed and verified at " + targetAppDir.getAbsolutePath());
            return;
        }

        Log.i(TAG, "Preparing standalone game bundle...");
        boolean success = false;

        // Method 1: High-speed direct streaming from APK zip package (~1-2 seconds)
        // Bypasses recursive AssetManager.list() overhead that causes Android ANR freezes.
        try {
            String apkPath = getApplicationInfo().sourceDir;
            if (apkPath != null && new File(apkPath).exists()) {
                Log.i(TAG, "Extracting bundled assets directly from APK: " + apkPath);
                try (ZipFile apkZip = new ZipFile(apkPath)) {
                    String prefix = "assets/" + APPS_DIR_NAME + "/" + APP_BUNDLE_NAME + "/";
                    Enumeration<? extends ZipEntry> entries = apkZip.entries();
                    byte[] buffer = new byte[65536];

                    while (entries.hasMoreElements()) {
                        ZipEntry entry = entries.nextElement();
                        String entryName = entry.getName();
                        if (entryName.startsWith(prefix)) {
                            String relPath = entryName.substring(prefix.length());
                            if (relPath.isEmpty()) continue;

                            File outFile = new File(targetAppDir, relPath);
                            if (entry.isDirectory()) {
                                if (!outFile.isDirectory() && !outFile.mkdirs()) {
                                    throw new IOException("Failed to create dir: " + outFile.getAbsolutePath());
                                }
                            } else {
                                File parent = outFile.getParentFile();
                                if (parent != null && !parent.exists() && !parent.mkdirs()) {
                                    throw new IOException("Failed to create parent dir: " + parent.getAbsolutePath());
                                }
                                try (InputStream is = apkZip.getInputStream(entry);
                                     FileOutputStream fos = new FileOutputStream(outFile)) {
                                    int len;
                                    while ((len = is.read(buffer)) > 0) {
                                        fos.write(buffer, 0, len);
                                    }
                                }
                            }
                        }
                    }

                    if (infoPlist.exists()) {
                        success = true;
                        Log.i(TAG, "Direct APK asset extraction completed successfully!");
                    }
                }
            }
        } catch (Throwable t) {
            Log.w(TAG, "Direct APK extraction failed, falling back to AssetManager: " + t.getMessage());
        }

        // Method 2: Try bundled archive (MapleCC.zip / MapleCC.ipa) if present
        if (!success) {
            AssetManager am = getAssets();
            String[] zipCandidates = new String[] {
                "MapleCC.zip",
                APPS_DIR_NAME + "/MapleCC.zip"
            };
            for (String zipPath : zipCandidates) {
                try (InputStream is = am.open(zipPath)) {
                    Log.i(TAG, "Extracting bundled game archive: " + zipPath);
                    unzip(is, appsDir);
                    if (infoPlist.exists()) {
                        success = true;
                        break;
                    }
                } catch (IOException ignored) {}
            }
        }

        // Method 3: Fallback loose asset copy via AssetManager
        if (!success) {
            AssetManager am = getAssets();
            String assetAppPath = APPS_DIR_NAME + "/" + APP_BUNDLE_NAME;
            try {
                String[] list = am.list(assetAppPath);
                if (list != null && list.length > 0) {
                    Log.i(TAG, "Copying loose bundled assets via AssetManager: " + assetAppPath);
                    copyAssetFolder(am, assetAppPath, targetAppDir);
                    if (infoPlist.exists()) {
                        success = true;
                    }
                }
            } catch (IOException e) {
                Log.e(TAG, "Failed reading asset folder: " + e.getMessage());
            }
        }

        // Write completion marker file only after complete extraction
        if (success && infoPlist.exists()) {
            try {
                if (!marker.exists()) {
                    marker.createNewFile();
                }
                Log.i(TAG, "Standalone installation verified and marker created.");
            } catch (IOException e) {
                Log.w(TAG, "Could not write installation marker: " + e.getMessage());
            }
        } else {
            Log.e(TAG, "Failed to verify complete installation of MapleCC.app");
        }
    }

    private void unzip(InputStream is, File targetDir) throws IOException {
        byte[] buffer = new byte[65536];
        try (ZipInputStream zis = new ZipInputStream(is)) {
            ZipEntry entry;
            while ((entry = zis.getNextEntry()) != null) {
                String entryName = entry.getName();
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
            try (InputStream is = am.open(srcPath)) {
                copyStream(is, dstDir);
            }
        } else {
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

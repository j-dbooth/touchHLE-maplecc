#! /bin/sh
TP=/usr/local/bin/TexturePacker

if [ "${ACTION}" = "clean" ]
then

# remove files

# remove Scene/Title
rm Art/Scene/Title/Title*.plist
rm Art/Scene/Title/Title*.pvr.ccz

# remove Scene/Battle
rm Art/Scene/Battle/BattleFX*.plist
rm Art/Scene/Battle/BattleFX*.pvr.ccz

# remove Scene/Navigation/Themes
rm Art/Scene/Navigation/Themes/*.plist
rm Art/Scene/Navigation/Themes/*.pvr.ccz

# remove Scene/Navigation
rm Art/Scene/Navigation/*.plist
rm Art/Scene/Navigation/*.pvr.ccz

# remove Scene/Town
rm Art/Scene/Town/*.plist
rm Art/Scene/Town/*.pvr.ccz

# remove Scene/Shop
rm Art/Scene/Shop/*.plist
rm Art/Scene/Shop/*.pvr.ccz

# remove Scene/Tavern
rm Art/Scene/Tavern/*.plist
rm Art/Scene/Tavern/*.pvr.ccz

# remove Scene/DungeonMap
rm Art/Scene/DungeonMap/*.plist
rm Art/Scene/DungeonMap/*.pvr.ccz

# remove Scene/DungeonEnter
rm Art/Scene/DungeonEnter/*.plist
rm Art/Scene/DungeonEnter/*.pvr.ccz

# remove Scene/Death
rm Art/Scene/Death/*.plist
rm Art/Scene/Death/*.pvr.ccz

# remove Scene/FloorResult
rm Art/Scene/FloorResult/*.plist
rm Art/Scene/FloorResult/*.pvr.ccz

# remove Scene/Credit
rm Art/Scene/Credit/*.plist
rm Art/Scene/Credit/*.pvr.ccz

# remove Scene/Option
rm Art/Scene/Option/*.plist
rm Art/Scene/Option/*.pvr.ccz

# remove Scene/Tutorial
rm Art/Scene/Tutorial/*.plist
rm Art/Scene/Tutorial/*.pvr.ccz

# remove Views/BattleBar
rm Art/Views/BattleBar/BattleBar*.plist
rm Art/Views/BattleBar/BattleBar*.pvr.ccz

# remove Views/BottomUI
rm Art/Views/BottomUI/BottomUI*.plist
rm Art/Views/BottomUI/BottomUI*.pvr.ccz

# remove Views/BattleTopUI
rm Art/Views/BattleTopUI/BattleTopUI*.plist
rm Art/Views/BattleTopUI/BattleTopUI*.pvr.ccz

# remove Views/NAVPlayer
rm Art/Views/NAVPlayer/*.plist
rm Art/Views/NAVPlayer/*.pvr.ccz

# remove Views/Portrait
rm Art/Views/Portrait/*.plist
rm Art/Views/Portrait/*.pvr.ccz

# remove Views/Minimap
rm Art/Views/Minimap/Minimap*.plist
rm Art/Views/Minimap/Minimap*.pvr.ccz

# remove Views/SkillGauge
rm Art/Views/SkillGauge/SkillGauge*.plist
rm Art/Views/SkillGauge/SkillGauge*.pvr.ccz

# remove Views/SideBookMenu
rm Art/Views/SideBookMenu/SideBookMenu*.plist
rm Art/Views/SideBookMenu/SideBookMenu*.pvr.ccz

# remove Views/BookMenu
rm Art/Views/BookMenu/BookMenu*.plist
rm Art/Views/BookMenu/BookMenu*.pvr.ccz

# remove Views/DungeonAchievement
rm Art/Views/DungeonAchievement/DungeonAchievement*.plist
rm Art/Views/DungeonAchievement/DungeonAchievement*.pvr.ccz

# remove Views/NoticeWindow
# rm Art/Views/NoticeWindow/NoticeWindow*.plist
# rm Art/Views/NoticeWindow/NoticeWindow*.pvr.ccz

# remove Monsters
rm Art/Monsters/*.plist
rm Art/Monsters/*.pvr.ccz

else

# create hd &amp; sd assets

# create Scene/Title
${TP} --smart-update ../Assets/Art/Scene/Title/*.png \
--auto-sd \
--padding 1 \
--format cocos2d \
--data Art/Scene/Title/Title-hd.plist \
--sheet Art/Scene/Title/Title-hd.pvr.ccz

# create Scene/Battle
${TP} --smart-update ../Assets/Art/Scene/Battle/BattleFX \
--auto-sd \
--extrude 1 \
--padding 1 \
--format cocos2d \
--data Art/Scene/Battle/BattleFX-hd.plist \
--sheet Art/Scene/Battle/BattleFX-hd.pvr.ccz

# create Scene/Navigation/Themes/Jungle
${TP} --smart-update ../Assets/Art/Scene/Navigation/Themes/Jungle \
--auto-sd \
--padding 1 \
--format cocos2d \
--data Art/Scene/Navigation/Themes/Theme_Jungle-hd.plist \
--sheet Art/Scene/Navigation/Themes/Theme_Jungle-hd.pvr.ccz

# create Scene/Navigation/Themes/HowlingMine
${TP} --smart-update ../Assets/Art/Scene/Navigation/Themes/HowlingMine \
--auto-sd \
--padding 1 \
--format cocos2d \
--data Art/Scene/Navigation/Themes/Theme_HowlingMine-hd.plist \
--sheet Art/Scene/Navigation/Themes/Theme_HowlingMine-hd.pvr.ccz

# create Scene/Navigation/Themes/SleepingForest
${TP} --smart-update ../Assets/Art/Scene/Navigation/Themes/SleepingForest \
--auto-sd \
--padding 1 \
--format cocos2d \
--data Art/Scene/Navigation/Themes/Theme_SleepingForest-hd.plist \
--sheet Art/Scene/Navigation/Themes/Theme_SleepingForest-hd.pvr.ccz

# create Scene/Navigation/Themes/DreamingTemple
${TP} --smart-update ../Assets/Art/Scene/Navigation/Themes/DreamingTemple \
--auto-sd \
--padding 1 \
--format cocos2d \
--data Art/Scene/Navigation/Themes/Theme_DreamingTemple-hd.plist \
--sheet Art/Scene/Navigation/Themes/Theme_DreamingTemple-hd.pvr.ccz

# create Scene/Navigation/Themes/ForgottenLab
${TP} --smart-update ../Assets/Art/Scene/Navigation/Themes/ForgottenLab \
--auto-sd \
--padding 1 \
--format cocos2d \
--data Art/Scene/Navigation/Themes/Theme_ForgottenLab-hd.plist \
--sheet Art/Scene/Navigation/Themes/Theme_ForgottenLab-hd.pvr.ccz

# create Scene/Navigation/Themes/FrozenMountains
${TP} --smart-update ../Assets/Art/Scene/Navigation/Themes/FrozenMountains \
--auto-sd \
--padding 1 \
--format cocos2d \
--data Art/Scene/Navigation/Themes/Theme_FrozenMountains-hd.plist \
--sheet Art/Scene/Navigation/Themes/Theme_FrozenMountains-hd.pvr.ccz

# create Scene/Navigation/
${TP} --smart-update ../Assets/Art/Scene/Navigation/*.png \
--auto-sd \
--padding 1 \
--format cocos2d \
--data Art/Scene/Navigation/NAV_Resource-hd.plist \
--sheet Art/Scene/Navigation/NAV_Resource-hd.pvr.ccz

# create Scene/Town
${TP} --smart-update ../Assets/Art/Scene/Town/*.png \
--auto-sd \
--padding 1 \
--format cocos2d \
--data Art/Scene/Town/Town_Resource-hd.plist \
--sheet Art/Scene/Town/Town_Resource-hd.pvr.ccz

# create Scene/Town/SleepingForest
${TP} --smart-update ../Assets/Art/Scene/Town/SleepingForest \
--auto-sd \
--padding 1 \
--format cocos2d \
--data Art/Scene/Town/Town_SleepingForest_Resource-hd.plist \
--sheet Art/Scene/Town/Town_SleepingForest_Resource-hd.pvr.ccz

# create Scene/Town/FrozenMountains
${TP} --smart-update ../Assets/Art/Scene/Town/FrozenMountains \
--auto-sd \
--padding 1 \
--format cocos2d \
--data Art/Scene/Town/Town_FrozenMountains_Resource-hd.plist \
--sheet Art/Scene/Town/Town_FrozenMountains_Resource-hd.pvr.ccz

# create Scene/Town/HowlingMine
${TP} --smart-update ../Assets/Art/Scene/Town/HowlingMine \
--auto-sd \
--padding 1 \
--format cocos2d \
--data Art/Scene/Town/Town_HowlingMine_Resource-hd.plist \
--sheet Art/Scene/Town/Town_HowlingMine_Resource-hd.pvr.ccz

# create Scene/Town/ForgottenLab
${TP} --smart-update ../Assets/Art/Scene/Town/ForgottenLab \
--auto-sd \
--padding 1 \
--format cocos2d \
--data Art/Scene/Town/Town_ForgottenLab_Resource-hd.plist \
--sheet Art/Scene/Town/Town_ForgottenLab_Resource-hd.pvr.ccz

# create Scene/Town/DreamingTemple
${TP} --smart-update ../Assets/Art/Scene/Town/DreamingTemple \
--auto-sd \
--padding 1 \
--format cocos2d \
--data Art/Scene/Town/Town_DreamingTemple_Resource-hd.plist \
--sheet Art/Scene/Town/Town_DreamingTemple_Resource-hd.pvr.ccz

# create Scene/Town/Default
${TP} --smart-update ../Assets/Art/Scene/Town/Default \
--auto-sd \
--padding 1 \
--format cocos2d \
--data Art/Scene/Town/Town_Default_Resource-hd.plist \
--sheet Art/Scene/Town/Town_Default_Resource-hd.pvr.ccz

# create Scene/Shop/
${TP} --smart-update ../Assets/Art/Scene/Shop/*.png \
--auto-sd \
--padding 1 \
--format cocos2d \
--data Art/Scene/Shop/Shop_Resource-hd.plist \
--sheet Art/Scene/Shop/Shop_Resource-hd.pvr.ccz

# create Scene/Tavern/
${TP} --smart-update ../Assets/Art/Scene/Tavern/*.png \
--auto-sd \
--padding 1 \
--format cocos2d \
--data Art/Scene/Tavern/Tavern_Resource-hd.plist \
--sheet Art/Scene/Tavern/Tavern_Resource-hd.pvr.ccz

# create Scene/DungeonMap/
${TP} --smart-update ../Assets/Art/Scene/DungeonMap/*.png \
--auto-sd \
--padding 1 \
--format cocos2d \
--data Art/Scene/DungeonMap/DungeonMap_Resource-hd.plist \
--sheet Art/Scene/DungeonMap/DungeonMap_Resource-hd.pvr.ccz

# create Scene/DungeonEnter/
${TP} --smart-update ../Assets/Art/Scene/DungeonEnter/*.png \
--auto-sd \
--padding 1 \
--format cocos2d \
--data Art/Scene/DungeonEnter/DungeonEnter_Resource-hd.plist \
--sheet Art/Scene/DungeonEnter/DungeonEnter_Resource-hd.pvr.ccz

# create Scene/Death/
${TP} --smart-update ../Assets/Art/Scene/Death/*.png \
--auto-sd \
--padding 1 \
--format cocos2d \
--data Art/Scene/Death/Death_Resource-hd.plist \
--sheet Art/Scene/Death/Death_Resource-hd.pvr.ccz

# create Scene/FloorResult/
${TP} --smart-update ../Assets/Art/Scene/FloorResult/*.png \
--auto-sd \
--padding 1 \
--format cocos2d \
--data Art/Scene/FloorResult/FloorResult_Resource-hd.plist \
--sheet Art/Scene/FloorResult/FloorResult_Resource-hd.pvr.ccz

# create Scene/Credit/
${TP} --smart-update ../Assets/Art/Scene/Credit/*.png \
--auto-sd \
--padding 1 \
--format cocos2d \
--data Art/Scene/Credit/Credit_Resource-hd.plist \
--sheet Art/Scene/Credit/Credit_Resource-hd.pvr.ccz

# create Scene/Option/
${TP} --smart-update ../Assets/Art/Scene/Option/*.png \
--auto-sd \
--padding 1 \
--format cocos2d \
--data Art/Scene/Option/Option_Resource-hd.plist \
--sheet Art/Scene/Option/Option_Resource-hd.pvr.ccz

# create Scene/Tutorial/English
${TP} --smart-update ../Assets/Art/Scene/Tutorial/English/*.png \
--auto-sd \
--padding 1 \
--format cocos2d \
--data Art/Scene/Tutorial/Tutorial_US_Resource-hd.plist \
--sheet Art/Scene/Tutorial/Tutorial_US_Resource-hd.pvr.ccz

# create Scene/Tutorial/Korean
${TP} --smart-update ../Assets/Art/Scene/Tutorial/Korean/*.png \
--auto-sd \
--padding 1 \
--format cocos2d \
--data Art/Scene/Tutorial/Tutorial_KR_Resource-hd.plist \
--sheet Art/Scene/Tutorial/Tutorial_KR_Resource-hd.pvr.ccz

# create Views/BattleBar
${TP} --smart-update ../Assets/Art/Views/BattleBar \
--auto-sd \
--extrude 1 \
--format cocos2d \
--data Art/Views/BattleBar/BattleBar-hd.plist \
--sheet Art/Views/BattleBar/BattleBar-hd.pvr.ccz

# create Views/BottomUI
${TP} --smart-update ../Assets/Art/Views/BottomUI \
--auto-sd \
--padding 1 \
--format cocos2d \
--data Art/Views/BottomUI/BottomUI-hd.plist \
--sheet Art/Views/BottomUI/BottomUI-hd.pvr.ccz

# create Views/BattleTopUI
${TP} --smart-update ../Assets/Art/Views/BattleTopUI \
--auto-sd \
--padding 1 \
--format cocos2d \
--data Art/Views/BattleTopUI/BattleTopUI-hd.plist \
--sheet Art/Views/BattleTopUI/BattleTopUI-hd.pvr.ccz

# create Views/NAVPlayer/FighterDefault
${TP} --smart-update ../Assets/Art/Views/NAVPlayer/FighterDefault \
--auto-sd \
--padding 1 \
--format cocos2d \
--data Art/Views/NAVPlayer/NAV_PC_FighterDefault-hd.plist \
--sheet Art/Views/NAVPlayer/NAV_PC_FighterDefault-hd.pvr.ccz

# create Views/NAVPlayer/ThiefDefault
${TP} --smart-update ../Assets/Art/Views/NAVPlayer/ThiefDefault \
--auto-sd \
--padding 1 \
--format cocos2d \
--data Art/Views/NAVPlayer/NAV_PC_ThiefDefault-hd.plist \
--sheet Art/Views/NAVPlayer/NAV_PC_ThiefDefault-hd.pvr.ccz

# create Views/NAVPlayer/Effects
${TP} --smart-update ../Assets/Art/Views/NAVPlayer/Effects/*.png \
--auto-sd \
--padding 1 \
--format cocos2d \
--data Art/Views/NAVPlayer/NAV_PC_Effects-hd.plist \
--sheet Art/Views/NAVPlayer/NAV_PC_Effects-hd.pvr.ccz

# create Views/Portrait/FighterDefault
${TP} --smart-update ../Assets/Art/Views/Portrait/FighterDefault \
--auto-sd \
--padding 1 \
--format cocos2d \
--data Art/Views/Portrait/BUI_Portrait_FighterDefault-hd.plist \
--sheet Art/Views/Portrait/BUI_Portrait_FighterDefault-hd.pvr.ccz

# create Views/Portrait/ThiefDefault
${TP} --smart-update ../Assets/Art/Views/Portrait/ThiefDefault \
--auto-sd \
--padding 1 \
--format cocos2d \
--data Art/Views/Portrait/BUI_Portrait_ThiefDefault-hd.plist \
--sheet Art/Views/Portrait/BUI_Portrait_ThiefDefault-hd.pvr.ccz

# create Views/Minimap
${TP} --smart-update ../Assets/Art/Views/Minimap \
--auto-sd \
--padding 1 \
--format cocos2d \
--data Art/Views/Minimap/Minimap-hd.plist \
--sheet Art/Views/Minimap/Minimap-hd.pvr.ccz

# create Views/SkillGauge
${TP} --smart-update ../Assets/Art/Views/SkillGauge/*.png \
--auto-sd \
--extrude 1 \
--format cocos2d \
--data Art/Views/SkillGauge/SkillGauge-hd.plist \
--sheet Art/Views/SkillGauge/SkillGauge-hd.pvr.ccz

# create Views/SideBookMenu
${TP} --smart-update ../Assets/Art/Views/SideBookMenu \
--auto-sd \
--extrude 1 \
--format cocos2d \
--data Art/Views/SideBookMenu/SideBookMenu-hd.plist \
--sheet Art/Views/SideBookMenu/SideBookMenu-hd.pvr.ccz

# create Views/BookMenu
${TP} --smart-update ../Assets/Art/Views/BookMenu \
--auto-sd \
--extrude 1 \
--format cocos2d \
--data Art/Views/BookMenu/BookMenu-hd.plist \
--sheet Art/Views/BookMenu/BookMenu-hd.pvr.ccz

# create Views/DungeonAchievement
${TP} --smart-update ../Assets/Art/Views/DungeonAchievement \
--auto-sd \
--extrude 1 \
--format cocos2d \
--data Art/Views/DungeonAchievement/DungeonAchievement-hd.plist \
--sheet Art/Views/DungeonAchievement/DungeonAchievement-hd.pvr.ccz

# create Views/NoticeWindow
# ${TP} --smart-update ../Assets/Art/Views/NoticeWindow \
# --auto-sd \
# --extrude 1 \
# --format cocos2d \
# --data Art/Views/NoticeWindow/NoticeWindow-hd.plist \
# --sheet Art/Views/NoticeWindow/NoticeWindow-hd.pvr.ccz

# create Monsters/SlimeJade
${TP} --smart-update ../Assets/Art/Monsters/SlimeJade \
--auto-sd \
--format cocos2d \
--data Art/Monsters/SlimeJade-hd.plist \
--sheet Art/Monsters/SlimeJade-hd.pvr.ccz

# create Monsters/SlimeGreen
${TP} --smart-update ../Assets/Art/Monsters/SlimeGreen \
--auto-sd \
--format cocos2d \
--data Art/Monsters/SlimeGreen-hd.plist \
--sheet Art/Monsters/SlimeGreen-hd.pvr.ccz

# create Monsters/MushroomBlue
${TP} --smart-update ../Assets/Art/Monsters/MushroomBlue \
--auto-sd \
--format cocos2d \
--data Art/Monsters/MushroomBlue-hd.plist \
--sheet Art/Monsters/MushroomBlue-hd.pvr.ccz

# create Monsters/Tauro
${TP} --smart-update ../Assets/Art/Monsters/Tauro \
--auto-sd \
--format cocos2d \
--data Art/Monsters/Tauro-hd.plist \
--sheet Art/Monsters/Tauro-hd.pvr.ccz

# create Monsters/Muru
${TP} --smart-update ../Assets/Art/Monsters/Muru \
--auto-sd \
--format cocos2d \
--data Art/Monsters/Muru-hd.plist \
--sheet Art/Monsters/Muru-hd.pvr.ccz

# create Monsters/BoneDog
${TP} --smart-update ../Assets/Art/Monsters/BoneDog \
--auto-sd \
--format cocos2d \
--data Art/Monsters/BoneDog-hd.plist \
--sheet Art/Monsters/BoneDog-hd.pvr.ccz

# create Monsters/Starpixy
${TP} --smart-update ../Assets/Art/Monsters/Starpixy \
--auto-sd \
--format cocos2d \
--data Art/Monsters/Starpixy-hd.plist \
--sheet Art/Monsters/Starpixy-hd.pvr.ccz

# create Monsters/Skeleton
${TP} --smart-update ../Assets/Art/Monsters/Skeleton \
--auto-sd \
--format cocos2d \
--data Art/Monsters/Skeleton-hd.plist \
--sheet Art/Monsters/Skeleton-hd.pvr.ccz

# create Monsters/Wraith
${TP} --smart-update ../Assets/Art/Monsters/Wraith \
--auto-sd \
--format cocos2d \
--data Art/Monsters/Wraith-hd.plist \
--sheet Art/Monsters/Wraith-hd.pvr.ccz

# create Monsters/Golem
${TP} --smart-update ../Assets/Art/Monsters/Golem \
--auto-sd \
--format cocos2d \
--data Art/Monsters/Golem-hd.plist \
--sheet Art/Monsters/Golem-hd.pvr.ccz

# create Monsters/Bear
${TP} --smart-update ../Assets/Art/Monsters/Bear \
--auto-sd \
--format cocos2d \
--data Art/Monsters/Bear-hd.plist \
--sheet Art/Monsters/Bear-hd.pvr.ccz

# create Monsters/Crocodile
${TP} --smart-update ../Assets/Art/Monsters/Crocodile \
--auto-sd \
--format cocos2d \
--data Art/Monsters/Crocodile-hd.plist \
--sheet Art/Monsters/Crocodile-hd.pvr.ccz

# create Monsters/Slime
${TP} --smart-update ../Assets/Art/Monsters/Slime \
--auto-sd \
--format cocos2d \
--data Art/Monsters/Slime-hd.plist \
--sheet Art/Monsters/Slime-hd.pvr.ccz

# create Monsters/Snail
${TP} --smart-update ../Assets/Art/Monsters/Snail \
--auto-sd \
--format cocos2d \
--data Art/Monsters/Snail-hd.plist \
--sheet Art/Monsters/Snail-hd.pvr.ccz

# create Monsters/Stump
${TP} --smart-update ../Assets/Art/Monsters/Stump \
--auto-sd \
--format cocos2d \
--data Art/Monsters/Stump-hd.plist \
--sheet Art/Monsters/Stump-hd.pvr.ccz

# create Monsters/Tino
${TP} --smart-update ../Assets/Art/Monsters/Tino \
--auto-sd \
--format cocos2d \
--data Art/Monsters/Tino-hd.plist \
--sheet Art/Monsters/Tino-hd.pvr.ccz

# create Monsters/Doraji
${TP} --smart-update ../Assets/Art/Monsters/Doraji \
--auto-sd \
--format cocos2d \
--data Art/Monsters/Doraji-hd.plist \
--sheet Art/Monsters/Doraji-hd.pvr.ccz

# create Monsters/Nependess
${TP} --smart-update ../Assets/Art/Monsters/Nependess \
--auto-sd \
--format cocos2d \
--data Art/Monsters/Nependess-hd.plist \
--sheet Art/Monsters/Nependess-hd.pvr.ccz

# create Monsters/Mask
${TP} --smart-update ../Assets/Art/Monsters/Mask \
--auto-sd \
--format cocos2d \
--data Art/Monsters/Mask-hd.plist \
--sheet Art/Monsters/Mask-hd.pvr.ccz

# create Monsters/Porcupine
${TP} --smart-update ../Assets/Art/Monsters/Porcupine \
--auto-sd \
--format cocos2d \
--data Art/Monsters/Porcupine-hd.plist \
--sheet Art/Monsters/Porcupine-hd.pvr.ccz

# create Monsters/Yeti
${TP} --smart-update ../Assets/Art/Monsters/Yeti \
--auto-sd \
--format cocos2d \
--data Art/Monsters/Yeti-hd.plist \
--sheet Art/Monsters/Yeti-hd.pvr.ccz

# create Monsters/Bulldog
${TP} --smart-update ../Assets/Art/Monsters/Bulldog \
--auto-sd \
--format cocos2d \
--data Art/Monsters/Bulldog-hd.plist \
--sheet Art/Monsters/Bulldog-hd.pvr.ccz

# create Monsters/Sellion
${TP} --smart-update ../Assets/Art/Monsters/Sellion \
--auto-sd \
--format cocos2d \
--data Art/Monsters/Sellion-hd.plist \
--sheet Art/Monsters/Sellion-hd.pvr.ccz

# create Monsters/Riti
${TP} --smart-update ../Assets/Art/Monsters/Riti \
--auto-sd \
--format cocos2d \
--data Art/Monsters/Riti-hd.plist \
--sheet Art/Monsters/Riti-hd.pvr.ccz

# create Monsters/Wyvern
${TP} --smart-update ../Assets/Art/Monsters/Wyvern \
--auto-sd \
--format cocos2d \
--data Art/Monsters/Wyvern-hd.plist \
--sheet Art/Monsters/Wyvern-hd.pvr.ccz

# create Monsters/Cleric
${TP} --smart-update ../Assets/Art/Monsters/Cleric \
--auto-sd \
--format cocos2d \
--data Art/Monsters/Cleric-hd.plist \
--sheet Art/Monsters/Cleric-hd.pvr.ccz

# create Monsters/Guardian
${TP} --smart-update ../Assets/Art/Monsters/Guardian \
--auto-sd \
--format cocos2d \
--data Art/Monsters/Guardian-hd.plist \
--sheet Art/Monsters/Guardian-hd.pvr.ccz

# create Monsters/Will
${TP} --smart-update ../Assets/Art/Monsters/Will \
--auto-sd \
--format cocos2d \
--data Art/Monsters/Will-hd.plist \
--sheet Art/Monsters/Will-hd.pvr.ccz

# create Monsters/WillO
${TP} --smart-update ../Assets/Art/Monsters/WillO \
--auto-sd \
--format cocos2d \
--data Art/Monsters/WillO-hd.plist \
--sheet Art/Monsters/WillO-hd.pvr.ccz

# create Monsters/WillOWisp
${TP} --smart-update ../Assets/Art/Monsters/WillOWisp \
--auto-sd \
--format cocos2d \
--data Art/Monsters/WillOWisp-hd.plist \
--sheet Art/Monsters/WillOWisp-hd.pvr.ccz

# create Monsters/KingSlime0
${TP} --smart-update ../Assets/Art/Monsters/KingSlime0 \
--auto-sd \
--format cocos2d \
--data Art/Monsters/KingSlime0-hd.plist \
--sheet Art/Monsters/KingSlime0-hd.pvr.ccz

# create Monsters/KingSlime1
${TP} --smart-update ../Assets/Art/Monsters/KingSlime1 \
--auto-sd \
--format cocos2d \
--data Art/Monsters/KingSlime1-hd.plist \
--sheet Art/Monsters/KingSlime1-hd.pvr.ccz

# create Monsters/KingSlime2
${TP} --smart-update ../Assets/Art/Monsters/KingSlime2 \
--auto-sd \
--format cocos2d \
--data Art/Monsters/KingSlime2-hd.plist \
--sheet Art/Monsters/KingSlime2-hd.pvr.ccz

# create Monsters/KingSlime3
${TP} --smart-update ../Assets/Art/Monsters/KingSlime3 \
--auto-sd \
--format cocos2d \
--data Art/Monsters/KingSlime3-hd.plist \
--sheet Art/Monsters/KingSlime3-hd.pvr.ccz

# create Monsters/KingSlime4
${TP} --smart-update ../Assets/Art/Monsters/KingSlime4 \
--auto-sd \
--format cocos2d \
--data Art/Monsters/KingSlime4-hd.plist \
--sheet Art/Monsters/KingSlime4-hd.pvr.ccz

# create Monsters/ClericGroup
${TP} --smart-update ../Assets/Art/Monsters/ClericGroup \
--auto-sd \
--format cocos2d \
--data Art/Monsters/ClericGroup-hd.plist \
--sheet Art/Monsters/ClericGroup-hd.pvr.ccz

# create Monsters/DarkRiti
${TP} --smart-update ../Assets/Art/Monsters/DarkRiti \
--auto-sd \
--format cocos2d \
--data Art/Monsters/DarkRiti-hd.plist \
--sheet Art/Monsters/DarkRiti-hd.pvr.ccz

# create Monsters/DarkYeti
${TP} --smart-update ../Assets/Art/Monsters/DarkYeti \
--auto-sd \
--format cocos2d \
--data Art/Monsters/DarkYeti-hd.plist \
--sheet Art/Monsters/DarkYeti-hd.pvr.ccz

# create Monsters/GuardianBlack
${TP} --smart-update ../Assets/Art/Monsters/GuardianBlack \
--auto-sd \
--format cocos2d \
--data Art/Monsters/GuardianBlack-hd.plist \
--sheet Art/Monsters/GuardianBlack-hd.pvr.ccz

# create Monsters/HornyMushroom
${TP} --smart-update ../Assets/Art/Monsters/HornyMushroom \
--auto-sd \
--format cocos2d \
--data Art/Monsters/HornyMushroom-hd.plist \
--sheet Art/Monsters/HornyMushroom-hd.pvr.ccz

# create Monsters/JuniorSellion
${TP} --smart-update ../Assets/Art/Monsters/JuniorSellion \
--auto-sd \
--format cocos2d \
--data Art/Monsters/JuniorSellion-hd.plist \
--sheet Art/Monsters/JuniorSellion-hd.pvr.ccz

# create Monsters/KingBlockGolem
${TP} --smart-update ../Assets/Art/Monsters/KingBlockGolem \
--auto-sd \
--format cocos2d \
--data Art/Monsters/KingBlockGolem-hd.plist \
--sheet Art/Monsters/KingBlockGolem-hd.pvr.ccz

# create Monsters/MechanicBear
${TP} --smart-update ../Assets/Art/Monsters/MechanicBear \
--auto-sd \
--format cocos2d \
--data Art/Monsters/MechanicBear-hd.plist \
--sheet Art/Monsters/MechanicBear-hd.pvr.ccz

# create Monsters/MechanicGolem
${TP} --smart-update ../Assets/Art/Monsters/MechanicGolem \
--auto-sd \
--format cocos2d \
--data Art/Monsters/MechanicGolem-hd.plist \
--sheet Art/Monsters/MechanicGolem-hd.pvr.ccz

# create Monsters/MechanicMask
${TP} --smart-update ../Assets/Art/Monsters/MechanicMask \
--auto-sd \
--format cocos2d \
--data Art/Monsters/MechanicMask-hd.plist \
--sheet Art/Monsters/MechanicMask-hd.pvr.ccz

# create Monsters/MechanicMushroomBlue
${TP} --smart-update ../Assets/Art/Monsters/MechanicMushroomBlue \
--auto-sd \
--format cocos2d \
--data Art/Monsters/MechanicMushroomBlue-hd.plist \
--sheet Art/Monsters/MechanicMushroomBlue-hd.pvr.ccz

# create Monsters/MechanicPig
${TP} --smart-update ../Assets/Art/Monsters/MechanicPig \
--auto-sd \
--format cocos2d \
--data Art/Monsters/MechanicPig-hd.plist \
--sheet Art/Monsters/MechanicPig-hd.pvr.ccz

# create Monsters/MechanicPixy
${TP} --smart-update ../Assets/Art/Monsters/MechanicPixy \
--auto-sd \
--format cocos2d \
--data Art/Monsters/MechanicPixy-hd.plist \
--sheet Art/Monsters/MechanicPixy-hd.pvr.ccz

# create Monsters/MechanicPorcupine
${TP} --smart-update ../Assets/Art/Monsters/MechanicPorcupine \
--auto-sd \
--format cocos2d \
--data Art/Monsters/MechanicPorcupine-hd.plist \
--sheet Art/Monsters/MechanicPorcupine-hd.pvr.ccz

# create Monsters/MechanicSlime
${TP} --smart-update ../Assets/Art/Monsters/MechanicSlime \
--auto-sd \
--format cocos2d \
--data Art/Monsters/MechanicSlime-hd.plist \
--sheet Art/Monsters/MechanicSlime-hd.pvr.ccz

# create Monsters/MechanicSnail
${TP} --smart-update ../Assets/Art/Monsters/MechanicSnail \
--auto-sd \
--format cocos2d \
--data Art/Monsters/MechanicSnail-hd.plist \
--sheet Art/Monsters/MechanicSnail-hd.pvr.ccz

# create Monsters/Mummydog
${TP} --smart-update ../Assets/Art/Monsters/Mummydog \
--auto-sd \
--format cocos2d \
--data Art/Monsters/Mummydog-hd.plist \
--sheet Art/Monsters/Mummydog-hd.pvr.ccz

# create Monsters/OfficerSkeleton
${TP} --smart-update ../Assets/Art/Monsters/OfficerSkeleton \
--auto-sd \
--format cocos2d \
--data Art/Monsters/OfficerSkeleton-hd.plist \
--sheet Art/Monsters/OfficerSkeleton-hd.pvr.ccz

# create Monsters/Shade
${TP} --smart-update ../Assets/Art/Monsters/Shade \
--auto-sd \
--format cocos2d \
--data Art/Monsters/Shade-hd.plist \
--sheet Art/Monsters/Shade-hd.pvr.ccz

# create Monsters/SilverSlime
${TP} --smart-update ../Assets/Art/Monsters/SilverSlime \
--auto-sd \
--format cocos2d \
--data Art/Monsters/SilverSlime-hd.plist \
--sheet Art/Monsters/SilverSlime-hd.pvr.ccz

# create Monsters/StoneMask
${TP} --smart-update ../Assets/Art/Monsters/StoneMask \
--auto-sd \
--format cocos2d \
--data Art/Monsters/StoneMask-hd.plist \
--sheet Art/Monsters/StoneMask-hd.pvr.ccz

# create Monsters/TauroDual
${TP} --smart-update ../Assets/Art/Monsters/TauroDual \
--auto-sd \
--format cocos2d \
--data Art/Monsters/TauroDual-hd.plist \
--sheet Art/Monsters/TauroDual-hd.pvr.ccz

# create Monsters/TauroSpear
${TP} --smart-update ../Assets/Art/Monsters/TauroSpear \
--auto-sd \
--format cocos2d \
--data Art/Monsters/TauroSpear-hd.plist \
--sheet Art/Monsters/TauroSpear-hd.pvr.ccz

# create Monsters/Tiguru
${TP} --smart-update ../Assets/Art/Monsters/Tiguru \
--auto-sd \
--format cocos2d \
--data Art/Monsters/Tiguru-hd.plist \
--sheet Art/Monsters/Tiguru-hd.pvr.ccz

# create Monsters/Timu
${TP} --smart-update ../Assets/Art/Monsters/Timu \
--auto-sd \
--format cocos2d \
--data Art/Monsters/Timu-hd.plist \
--sheet Art/Monsters/Timu-hd.pvr.ccz

# create Monsters/FlyEye
${TP} --smart-update ../Assets/Art/Monsters/FlyEye \
--auto-sd \
--format cocos2d \
--data Art/Monsters/FlyEye-hd.plist \
--sheet Art/Monsters/FlyEye-hd.pvr.ccz

# create Monsters/MechanicFlyEye
${TP} --smart-update ../Assets/Art/Monsters/MechanicFlyEye \
--auto-sd \
--format cocos2d \
--data Art/Monsters/MechanicFlyEye-hd.plist \
--sheet Art/Monsters/MechanicFlyEye-hd.pvr.ccz

# create Monsters/WraithJr
${TP} --smart-update ../Assets/Art/Monsters/WraithJr \
--auto-sd \
--format cocos2d \
--data Art/Monsters/WraithJr-hd.plist \
--sheet Art/Monsters/WraithJr-hd.pvr.ccz

# create Monsters/MadScientist
${TP} --smart-update ../Assets/Art/Monsters/MadScientist \
--auto-sd \
--format cocos2d \
--data Art/Monsters/MadScientist-hd.plist \
--sheet Art/Monsters/MadScientist-hd.pvr.ccz

# create Monsters/JuniorBalrog
${TP} --smart-update ../Assets/Art/Monsters/JuniorBalrog \
--auto-sd \
--format cocos2d \
--data Art/Monsters/JuniorBalrog-hd.plist \
--sheet Art/Monsters/JuniorBalrog-hd.pvr.ccz

# create Monsters/Lich
${TP} --smart-update ../Assets/Art/Monsters/Lich \
--auto-sd \
--format cocos2d \
--data Art/Monsters/Lich-hd.plist \
--sheet Art/Monsters/Lich-hd.pvr.ccz

# create Monsters/BookMaster
${TP} --smart-update ../Assets/Art/Monsters/BookMaster \
--auto-sd \
--format cocos2d \
--data Art/Monsters/BookMaster-hd.plist \
--sheet Art/Monsters/BookMaster-hd.pvr.ccz

# create Monsters/Pig
${TP} --smart-update ../Assets/Art/Monsters/Pig \
--auto-sd \
--format cocos2d \
--data Art/Monsters/Pig-hd.plist \
--sheet Art/Monsters/Pig-hd.pvr.ccz

fi

exit 0
"""Spread card and revision timestamps of a freshly seeded database so the app looks lived-in.

Usage: python dates.py <data-dir>/cardfile.db
"""

import sqlite3, sys, time

con = sqlite3.connect(sys.argv[1])
rows = con.execute("select id, title from cards order by created_at desc, rowid desc").fetchall()
now = int(time.time()*1000)
for i,(cid,title) in enumerate(rows):
    upd = now - int((i*1.7 + 0.1)*86400000) - i*3600000*3
    cre = upd - (i+2)*86400000*4
    con.execute("update cards set updated_at=?, created_at=? where id=?", (upd, cre, cid))
    print(i, title)
con.commit()
revs = con.execute("select id from card_revisions order by created_at, rowid").fetchall()
for i,(rid,) in enumerate(revs):
    con.execute("update card_revisions set created_at=? where id=?", (now - (len(revs)-i)*86400000*2 - 5400000*i, rid))
con.commit()

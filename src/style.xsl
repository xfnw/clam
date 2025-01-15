<?xml version="1.0" encoding="UTF-8"?>
<xsl:stylesheet version="1.0"
	xmlns="http://www.w3.org/1999/xhtml"
	xmlns:xsl="http://www.w3.org/1999/XSL/Transform"
	xmlns:atom="http://www.w3.org/2005/Atom">

<xsl:template match="/atom:feed">
	<html>
	<head>
		<meta charset="UTF-8"/>
		<meta name="viewport" content="width=device-width, initial-scale=1"/>
		<link rel="stylesheet" href="style.css"/>
		<title><xsl:value-of select="atom:title"/></title>
	</head>
	<body>
		<h1><xsl:value-of select="atom:title"/></h1>
		<xsl:for-each select="atom:entry">
		<article>
			<h2><a>
				<xsl:attribute name="href">
					<xsl:value-of select="atom:link[@rel='alternate']/@href"/>
				</xsl:attribute>
				<xsl:value-of select="atom:title"/>
			</a></h2>
			<small>
				updated
				<time>
					<xsl:attribute name="datetime">
						<xsl:value-of select="atom:updated"/>
					</xsl:attribute>
					<xsl:value-of select="substring(atom:updated,1,10)"/>
				</time>
				by
				<i>
					<xsl:value-of select="atom:author"/>
				</i>
			</small>
			<pre><xsl:value-of select="atom:summary"/></pre>
		</article>
		</xsl:for-each>
	</body>
	</html>
</xsl:template>

</xsl:stylesheet>
